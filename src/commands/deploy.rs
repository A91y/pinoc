use super::artifact::{
    check_built_for_cluster, check_entrypoint, read_build_record, read_manifest, sbpf_version,
    sha256_hex, Rebuild, Recorded,
};
use super::keys::find_program_id_decl;
use crate::config::{self, ClusterConfig, PinocConfig};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct DeployArgs<'a> {
    pub cluster: Option<&'a str>,
    pub wallet: Option<&'a str>,
    pub program_keypair: Option<&'a str>,
    pub upgrade_authority: Option<&'a str>,
}

/// Where a deploy or verify goes, from the flags, `[clusters]` and `[provider]`
/// in `Pinoc.toml`, and `solana config get`, in that order.
struct Target {
    /// `solana --url`.
    url: String,
    /// The `[clusters]` entry `--cluster` (or `[provider] cluster`) named,
    /// by name or by its `url`.
    profile: Option<String>,
    clusters: BTreeMap<String, ClusterConfig>,
    wallet: String,
    upgrade_authority: Option<String>,
    program_keypair: Option<String>,
}

fn resolve_target(
    cluster: Option<&str>,
    wallet: Option<&str>,
    upgrade_authority: Option<&str>,
) -> Result<Target> {
    let config = config::read_pinoc_config_optional()?;
    let (default_cluster, default_wallet) = match &config {
        Some(c) => (c.provider.cluster.clone(), c.provider.wallet.clone()),
        None if cluster.is_some() && wallet.is_some() => (String::new(), String::new()),
        None => {
            println!("   Falling back to `solana config get` for cluster/wallet defaults.");
            read_solana_cli_config()?
        }
    };
    let asked = cluster.map(str::to_string).unwrap_or(default_cluster);
    let clusters = config
        .as_ref()
        .map(|c| c.clusters.clone())
        .unwrap_or_default();
    let profile = find_profile(&asked, &clusters);
    let entry = profile.as_ref().and_then(|p| clusters.get(p));
    let url = entry
        .and_then(|e| e.url.clone())
        .unwrap_or_else(|| asked.clone());
    let wallet = wallet
        .map(str::to_string)
        .or_else(|| entry.and_then(|e| e.wallet.clone()))
        .unwrap_or(default_wallet);
    let provider = config.as_ref().map(|c: &PinocConfig| &c.provider);
    let upgrade_authority = upgrade_authority
        .map(str::to_string)
        .or_else(|| entry.and_then(|e| e.upgrade_authority.clone()))
        .or_else(|| provider.and_then(|p| p.upgrade_authority.clone()));
    let program_keypair = provider.and_then(|p| p.program_keypair.clone());
    Ok(Target {
        url,
        profile,
        clusters,
        wallet,
        upgrade_authority,
        program_keypair,
    })
}

/// The `[clusters]` entry called `asked`, else the one whose `url` is `asked`.
fn find_profile(asked: &str, clusters: &BTreeMap<String, ClusterConfig>) -> Option<String> {
    if clusters.contains_key(asked) {
        return Some(asked.to_string());
    }
    clusters
        .iter()
        .find(|(_, c)| c.url.as_deref() == Some(asked))
        .map(|(name, _)| name.clone())
}

/// The package's own artifact, else the only `.so` in `target/deploy`.
fn find_artifact() -> Result<PathBuf> {
    let target_deploy_dir = Path::new("target/deploy");
    if !target_deploy_dir.exists() {
        anyhow::bail!("target/deploy directory not found. Please run 'pinoc build' first.");
    }
    if let Some(path) = read_manifest()
        .as_ref()
        .and_then(super::artifact::artifact_path)
    {
        if path.exists() {
            return Ok(path);
        }
        anyhow::bail!(
            "{} not found. Please run 'pinoc build' first.",
            path.display()
        );
    }
    let mut found: Vec<PathBuf> = fs::read_dir(target_deploy_dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("so"))
        .collect();
    found.sort();
    match found.len() {
        0 => anyhow::bail!("No .so file found in target/deploy. Please run 'pinoc build' first."),
        1 => Ok(found.remove(0)),
        _ => anyhow::bail!(
            "target/deploy holds {} .so files ({}) and there is no Cargo.toml to say which is this program's.",
            found.len(),
            found
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// The address `keypair` (a Solana CLI keypair file) holds.
pub fn keypair_address(keypair: &Path) -> Result<String> {
    let content = fs::read_to_string(keypair)
        .with_context(|| format!("Failed to read {}", keypair.display()))?;
    let bytes: Vec<u8> = serde_json::from_str(&content)
        .with_context(|| format!("{} is not a Solana keypair file", keypair.display()))?;
    if bytes.len() != 64 {
        anyhow::bail!(
            "{} is not a Solana keypair file: it holds {} bytes, not 64",
            keypair.display(),
            bytes.len()
        );
    }
    Ok(bs58::encode(&bytes[32..]).into_string())
}

/// What `solana program deploy --program-id` is given.
enum ProgramId {
    /// The program's keypair, which a first deploy needs.
    Keypair(PathBuf),
    /// Only the address: enough to upgrade a program that exists.
    Address(String),
}

/// The keypair given with `--program-keypair` or `[provider] program_keypair`,
/// else `target/deploy/<name>-keypair.json`, refused unless it holds the
/// address the program declares. `cargo build-sbf` writes a fresh keypair at
/// the default path when there is none, so a fresh checkout would otherwise
/// deploy to an address nothing refers to.
fn resolve_program_id(
    so_path: &Path,
    flag: Option<&str>,
    configured: Option<&str>,
) -> Result<ProgramId> {
    let declared = if Path::new("src").exists() {
        find_program_id_decl(Path::new("src"))?
    } else {
        None
    };
    let default = so_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|stem| so_path.with_file_name(format!("{stem}-keypair.json")));
    let (keypair, source) = match (flag, configured) {
        (Some(path), _) => (
            Some(PathBuf::from(expand_tilde(path)?)),
            "--program-keypair",
        ),
        (None, Some(path)) => (
            Some(PathBuf::from(expand_tilde(path)?)),
            "[provider] program_keypair",
        ),
        (None, None) => (default.filter(|p| p.exists()), "default"),
    };

    match (keypair, declared) {
        (Some(keypair), declared) => {
            if !keypair.exists() {
                anyhow::bail!(
                    "{} (from {source}) does not exist.",
                    keypair.display()
                );
            }
            let holds = keypair_address(&keypair)?;
            if let Some(decl) = &declared {
                if decl.address != holds {
                    let mut msg = format!(
                        "{} holds {holds}, but {} declares {}. Deploying it would put the program at an address nothing refers to.",
                        keypair.display(),
                        decl.file.display(),
                        decl.address
                    );
                    if source == "default" {
                        msg.push_str(&format!(
                            "\n\n`cargo build-sbf` writes a new keypair at {} when there is none, so this one is probably not the program's. Point `[provider] program_keypair` in Pinoc.toml (or `--program-keypair`) at the keypair for {}. If this is a new program that should live at {holds}, run `pinoc keys sync` to declare it.",
                            keypair.display(),
                            decl.address
                        ));
                    } else {
                        msg.push_str(&format!(
                            "\n\nFix the keypair given by {source}, or, if the program is meant to move to {holds}, run `pinoc keys sync`."
                        ));
                    }
                    anyhow::bail!(msg);
                }
            } else {
                println!(
                    "⚠️  No program ID declaration found under src/, so {} cannot be checked against one.",
                    keypair.display()
                );
            }
            println!("   Program keypair: {} ({holds})", keypair.display());
            Ok(ProgramId::Keypair(keypair))
        }
        (None, Some(decl)) => {
            println!(
                "   No program keypair; upgrading the program at {} (declared in {}). A first deploy needs its keypair: set `[provider] program_keypair` or pass --program-keypair.",
                decl.address,
                decl.file.display()
            );
            Ok(ProgramId::Address(decl.address))
        }
        (None, None) => anyhow::bail!(
            "Nothing names this program's address: there is no program keypair and no `declare_id!` or `const ID` under src/, so `solana program deploy` would put it at a new random address. Set `[provider] program_keypair` in Pinoc.toml or pass --program-keypair."
        ),
    }
}

pub fn run_deploy(args: DeployArgs) -> Result<()> {
    println!("Deploying program");

    let target = resolve_target(args.cluster, args.wallet, args.upgrade_authority)?;

    println!("📋 Using configuration:");
    match &target.profile {
        Some(profile) => println!("   Cluster: {} (`[clusters.{profile}]`)", target.url),
        None => println!("   Cluster: {}", target.url),
    }
    println!("   Wallet: {}", target.wallet);

    let so_path = find_artifact()?;
    check_entrypoint(&so_path, read_manifest().as_ref(), Rebuild::Deploy)?;
    check_sbpf_version(&so_path, &target.url)?;
    if let Some(profile) = &target.profile {
        check_built_for_cluster(&so_path, profile, &target.clusters)?;
    }
    let program_id = resolve_program_id(
        &so_path,
        args.program_keypair,
        target.program_keypair.as_deref(),
    )?;

    let mut deploy_cmd = Command::new("solana");
    deploy_cmd
        .arg("program")
        .arg("deploy")
        .arg("--url")
        .arg(&target.url)
        .arg("--keypair")
        .arg(expand_tilde(&target.wallet)?);
    match &program_id {
        ProgramId::Keypair(path) => deploy_cmd.arg("--program-id").arg(path),
        ProgramId::Address(address) => deploy_cmd.arg("--program-id").arg(address),
    };
    if let Some(authority) = &target.upgrade_authority {
        println!("   Upgrade authority: {authority}");
        deploy_cmd
            .arg("--upgrade-authority")
            .arg(expand_tilde(authority)?);
    }
    deploy_cmd.arg(&so_path);

    let status = deploy_cmd
        .spawn()?
        .wait()
        .with_context(|| "Failed to deploy program")?;

    if !status.success() {
        anyhow::bail!("Deploy failed with exit code: {:?}", status.code());
    } else {
        println!("Program deployed successfully!");
        let cluster = target.profile.as_deref().unwrap_or(&target.url);
        println!("   Check what is on chain with: pinoc verify --cluster {cluster}");
    }

    Ok(())
}

pub struct VerifyArgs<'a> {
    pub cluster: Option<&'a str>,
    pub program_id: Option<&'a str>,
}

/// Downloads the deployed program and compares it with the local artifact.
pub fn run_verify(args: VerifyArgs) -> Result<()> {
    let target = resolve_target(args.cluster, None, None)?;
    let address = match args.program_id {
        Some(address) => address.to_string(),
        None => {
            let declared = if Path::new("src").exists() {
                find_program_id_decl(Path::new("src"))?
            } else {
                None
            };
            match declared {
                Some(decl) => decl.address,
                None => anyhow::bail!(
                    "No program ID declaration found under src/. Pass --program-id <ADDRESS>."
                ),
            }
        }
    };
    let so_path = find_artifact()?;
    let local =
        fs::read(&so_path).with_context(|| format!("Failed to read {}", so_path.display()))?;
    println!(
        "Verifying {address} on {} against {}",
        target.url,
        so_path.display()
    );
    if let Some(profile) = &target.profile {
        check_built_for_cluster(&so_path, profile, &target.clusters)?;
    }

    let dump =
        std::env::temp_dir().join(format!("pinoc-verify-{}-{address}.so", std::process::id()));
    let output = Command::new("solana")
        .args(["program", "dump", &address])
        .arg(&dump)
        .args(["--url", &target.url])
        .output()
        .with_context(|| "Failed to run 'solana program dump'. Is the Solana CLI installed?")?;
    if !output.status.success() {
        let _ = fs::remove_file(&dump);
        anyhow::bail!(
            "'solana program dump {address}' failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let onchain = fs::read(&dump).with_context(|| format!("Failed to read {}", dump.display()));
    let _ = fs::remove_file(&dump);
    let onchain = onchain?;

    let local_line = format!("   local    {}  {} bytes", sha256_hex(&local), local.len());
    let program = deployed_program(&onchain);
    let mut onchain_line = format!(
        "   on chain {}  {} bytes",
        sha256_hex(program),
        program.len()
    );
    if onchain.len() > program.len() {
        onchain_line.push_str(&format!(
            " (the program account holds {}; the rest is unused space, all zero)",
            onchain.len()
        ));
    }
    println!("{local_line}");
    if let Recorded::Yes(record) = read_build_record(&so_path) {
        let features = if record.features.is_empty() {
            "no features".to_string()
        } else {
            format!("--features {}", record.features.join(","))
        };
        let cluster = record
            .cluster
            .map(|c| format!(", for cluster `{c}`"))
            .unwrap_or_default();
        println!("   built by pinoc with {features}{cluster}");
    }
    println!("{onchain_line}");
    match compare_deployed(&local, &onchain) {
        Deployed::Same => {
            println!("✅ The deployed program is {}", so_path.display());
            Ok(())
        }
        Deployed::Different => {
            anyhow::bail!("The program at {address} is not {}.", so_path.display())
        }
    }
}

#[derive(Debug, PartialEq)]
enum Deployed {
    Same,
    Different,
}

/// The program account can be larger than the program (`--max-len`, or an
/// upgrade to a smaller one), and the space past it is zero.
fn compare_deployed(local: &[u8], onchain: &[u8]) -> Deployed {
    if onchain.len() >= local.len()
        && onchain[..local.len()] == *local
        && onchain[local.len()..].iter().all(|b| *b == 0)
    {
        Deployed::Same
    } else {
        Deployed::Different
    }
}

/// The program in a program account's data: the ELF file its headers describe,
/// when only zeros follow, else the whole account. A program's own last bytes
/// can be zero, so trailing zeros do not mark where it ends.
fn deployed_program(account: &[u8]) -> &[u8] {
    match elf_extent(account) {
        Some(end) if account[end..].iter().all(|b| *b == 0) => &account[..end],
        _ => account,
    }
}

/// The length of the ELF64 little-endian file at the start of `bytes`: the
/// furthest end of its header tables, sections and segments.
fn elf_extent(bytes: &[u8]) -> Option<usize> {
    let u16_at = |o: usize| Some(u16::from_le_bytes(bytes.get(o..o + 2)?.try_into().ok()?));
    let u32_at = |o: usize| Some(u32::from_le_bytes(bytes.get(o..o + 4)?.try_into().ok()?));
    let u64_at = |o: usize| Some(u64::from_le_bytes(bytes.get(o..o + 8)?.try_into().ok()?));
    const SHT_NOBITS: u32 = 8;

    if bytes.get(0..6)? != b"\x7fELF\x02\x01" {
        return None;
    }
    let phoff = u64_at(0x20)?;
    let shoff = u64_at(0x28)?;
    let (phentsize, phnum) = (u64::from(u16_at(0x36)?), u64::from(u16_at(0x38)?));
    let (shentsize, shnum) = (u64::from(u16_at(0x3a)?), u64::from(u16_at(0x3c)?));
    let mut end = 0x40u64
        .max(phoff.checked_add(phentsize * phnum)?)
        .max(shoff.checked_add(shentsize * shnum)?);
    for i in 0..shnum {
        let header = usize::try_from(shoff + i * shentsize).ok()?;
        if u32_at(header + 0x04)? != SHT_NOBITS {
            end = end.max(u64_at(header + 0x18)?.checked_add(u64_at(header + 0x20)?)?);
        }
    }
    for i in 0..phnum {
        let header = usize::try_from(phoff + i * phentsize).ok()?;
        end = end.max(u64_at(header + 0x08)?.checked_add(u64_at(header + 0x20)?)?);
    }
    usize::try_from(end).ok().filter(|end| *end <= bytes.len())
}

/// SIMD-0178, SIMD-0189 and SIMD-0377: enable deployment and execution of SBPFv3.
const ENABLE_SBPF_V3: &str = "5cC3foj77CWun58pC51ebHFUWavHWKarWyR5UUik7dnC";
/// SIMD-0500: disable deployment of SBPF v0, v1 and v2.
const DISABLE_SBPF_V0_V1_V2: &str = "B8JJXCy5amZyWG9r7EnUYLwzXSXTxG7GZ1qZ1qggo83g";

/// Which of the two SBPF deployment gates are active on a cluster.
#[derive(Debug, Clone, Copy, PartialEq)]
struct SbpfGates {
    v3_enabled: bool,
    older_disabled: bool,
}

/// Refuses an artifact the cluster's loader would reject for its SBPF version.
/// The loader reports both directions as "Detected sbpf_version required by
/// the executable which are not enabled", after the buffer has been paid for.
/// If the cluster cannot be asked, the deploy goes ahead unchecked.
fn check_sbpf_version(so_path: &Path, cluster_url: &str) -> Result<()> {
    let Some(version) = fs::read(so_path).ok().as_deref().and_then(sbpf_version) else {
        return Ok(());
    };
    let Some(gates) = read_sbpf_gates(cluster_url) else {
        return Ok(());
    };
    match sbpf_refusal(version, gates) {
        Some(reason) => anyhow::bail!(
            "{} is SBPFv{version}; cluster {cluster_url} {reason}",
            so_path.display()
        ),
        None => Ok(()),
    }
}

fn sbpf_refusal(version: u32, gates: SbpfGates) -> Option<&'static str> {
    if version < 3 && gates.older_disabled {
        return Some("has SIMD-0500 active and does not deploy SBPF v0, v1 or v2. Rebuild with `pinoc build --arch v3`, or set `arch = \"v3\"` under `[build]` in Pinoc.toml.");
    }
    if version == 3 && !gates.v3_enabled {
        return Some("has not enabled SBPFv3. Rebuild with `pinoc build --arch v0`.");
    }
    None
}

fn read_sbpf_gates(cluster_url: &str) -> Option<SbpfGates> {
    let output = Command::new("solana")
        .args(["feature", "status", ENABLE_SBPF_V3, DISABLE_SBPF_V0_V1_V2])
        .args(["--url", cluster_url, "--output", "json"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_sbpf_gates(&String::from_utf8_lossy(&output.stdout))
}

fn parse_sbpf_gates(json: &str) -> Option<SbpfGates> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let features = value.get("features")?.as_array()?;
    let active = |id: &str| {
        let feature = features
            .iter()
            .find(|f| f.get("id").and_then(|i| i.as_str()) == Some(id))?;
        Some(feature.get("status")?.as_str()? == "active")
    };
    Some(SbpfGates {
        v3_enabled: active(ENABLE_SBPF_V3)?,
        older_disabled: active(DISABLE_SBPF_V0_V1_V2)?,
    })
}

fn read_solana_cli_config() -> Result<(String, String)> {
    let output = Command::new("solana")
        .arg("config")
        .arg("get")
        .output()
        .with_context(|| "Failed to run 'solana config get'. Is the Solana CLI installed?")?;

    if !output.status.success() {
        anyhow::bail!(
            "'solana config get' failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let cluster_url = extract_solana_config_field(&stdout, "RPC URL:").ok_or_else(|| {
        anyhow::anyhow!("Could not find 'RPC URL:' in 'solana config get' output")
    })?;
    let wallet_path = extract_solana_config_field(&stdout, "Keypair Path:").ok_or_else(|| {
        anyhow::anyhow!("Could not find 'Keypair Path:' in 'solana config get' output")
    })?;

    Ok((cluster_url, wallet_path))
}

fn extract_solana_config_field(output: &str, prefix: &str) -> Option<String> {
    output
        .lines()
        .find(|line| line.trim_start().starts_with(prefix))
        .map(|line| line.trim_start_matches(prefix).trim().to_string())
}

fn expand_tilde(path: &str) -> Result<String> {
    if path.starts_with("~") {
        if let Some(home_dir) = dirs::home_dir() {
            return Ok(path.replacen("~", home_dir.to_str().unwrap_or(""), 1));
        } else {
            anyhow::bail!("Could not determine the home directory to expand '~'");
        }
    }
    Ok(path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gates(v3_enabled: bool, older_disabled: bool) -> SbpfGates {
        SbpfGates {
            v3_enabled,
            older_disabled,
        }
    }

    #[test]
    fn an_artifact_is_refused_only_where_the_loader_would_refuse_it() {
        for version in 0..3 {
            assert!(sbpf_refusal(version, gates(true, false)).is_none());
            assert!(sbpf_refusal(version, gates(false, false)).is_none());
            let reason = sbpf_refusal(version, gates(true, true)).unwrap();
            assert!(reason.contains("SIMD-0500") && reason.contains("--arch v3"));
        }
        assert!(sbpf_refusal(3, gates(true, false)).is_none());
        assert!(sbpf_refusal(3, gates(true, true)).is_none());
        assert!(sbpf_refusal(3, gates(false, false))
            .unwrap()
            .contains("--arch v0"));
        assert!(sbpf_refusal(4, gates(true, true)).is_none());
    }

    #[test]
    fn a_deployed_program_may_be_followed_only_by_zeros() {
        let local = [1, 2, 0, 3, 0];
        assert_eq!(compare_deployed(&local, &local), Deployed::Same);
        assert_eq!(
            compare_deployed(&local, &[1, 2, 0, 3, 0, 0, 0]),
            Deployed::Same
        );
        assert_eq!(
            compare_deployed(&local, &[1, 2, 0, 3, 0, 0, 9]),
            Deployed::Different
        );
        assert_eq!(compare_deployed(&local, &[1, 2, 0, 3]), Deployed::Different);
        assert_eq!(
            compare_deployed(&local, &[1, 2, 0, 4, 0]),
            Deployed::Different
        );
    }

    /// An ELF64 header followed by a one-entry section header table, which
    /// ends in zero bytes the way an SBF program's does.
    fn elf_with_trailing_zeros() -> Vec<u8> {
        let mut elf = vec![0u8; 0x80];
        elf[..6].copy_from_slice(b"\x7fELF\x02\x01");
        elf[0x28..0x30].copy_from_slice(&0x40u64.to_le_bytes());
        elf[0x3a..0x3c].copy_from_slice(&0x40u16.to_le_bytes());
        elf[0x3c..0x3e].copy_from_slice(&1u16.to_le_bytes());
        elf[0x44..0x48].copy_from_slice(&1u32.to_le_bytes());
        elf[0x58..0x60].copy_from_slice(&0x40u64.to_le_bytes());
        elf[0x60..0x68].copy_from_slice(&0x20u64.to_le_bytes());
        elf
    }

    #[test]
    fn the_deployed_program_ends_where_its_elf_ends_not_at_its_last_nonzero_byte() {
        let program = elf_with_trailing_zeros();
        assert_eq!(elf_extent(&program), Some(program.len()));
        let mut account = program.clone();
        account.extend([0u8; 100]);
        assert_eq!(deployed_program(&account), &program[..]);

        let mut longer = program.clone();
        longer[0x60..0x68].copy_from_slice(&0x60u64.to_le_bytes());
        assert_eq!(elf_extent(&longer), None);
        account.push(1);
        assert_eq!(deployed_program(&account), &account[..]);
        assert_eq!(deployed_program(&[0, 0, 1]), &[0, 0, 1]);
    }

    #[test]
    fn a_keypair_file_holds_its_address_in_its_second_half() {
        let path = std::env::temp_dir().join(format!("pinoc-keypair-{}.json", std::process::id()));
        // The system program's address is 32 zero bytes.
        let mut bytes = vec![5u8; 32];
        bytes.extend([0u8; 32]);
        fs::write(&path, serde_json::to_string(&bytes).unwrap()).unwrap();
        assert_eq!(
            keypair_address(&path).unwrap(),
            "11111111111111111111111111111111"
        );
        fs::write(&path, "[1, 2, 3]").unwrap();
        assert!(keypair_address(&path)
            .unwrap_err()
            .to_string()
            .contains("holds 3 bytes, not 64"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn gates_are_read_from_feature_status_output() {
        let json = |v3: &str, older: &str| {
            format!(
                r#"{{"features":[{{"id":"{ENABLE_SBPF_V3}","status":"{v3}","sinceSlot":0}},{{"id":"{DISABLE_SBPF_V0_V1_V2}","status":"{older}"}}]}}"#
            )
        };
        assert_eq!(
            parse_sbpf_gates(&json("active", "inactive")),
            Some(gates(true, false))
        );
        assert_eq!(
            parse_sbpf_gates(&json("active", "active")),
            Some(gates(true, true))
        );
        assert_eq!(
            parse_sbpf_gates(&json("pending", "inactive")),
            Some(gates(false, false))
        );
        assert_eq!(parse_sbpf_gates(r#"{"features":[]}"#), None);
        assert_eq!(parse_sbpf_gates("Error: connection refused"), None);
    }
}
