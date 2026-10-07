use super::artifact::{check_entrypoint, read_manifest, sbpf_version, Rebuild};
use crate::config;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn run_deploy(cluster: Option<&str>, wallet: Option<&str>) -> Result<()> {
    println!("Deploying program");

    let (default_cluster, default_wallet) = resolve_defaults(cluster, wallet)?;
    let cluster_url = cluster.unwrap_or(&default_cluster);
    let wallet_path = wallet.unwrap_or(&default_wallet);

    println!("📋 Using configuration:");
    println!("   Cluster: {}", cluster_url);
    println!("   Wallet: {}", wallet_path);

    let target_deploy_dir = Path::new("target/deploy");
    if !target_deploy_dir.exists() {
        anyhow::bail!("target/deploy directory not found. Please run 'pinoc build' first.");
    }

    let mut so_file = None;
    for entry in fs::read_dir(target_deploy_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("so") {
            so_file = Some(path);
            break;
        }
    }

    let so_path = so_file.ok_or_else(|| {
        anyhow::anyhow!("No .so file found in target/deploy. Please run 'pinoc build' first.")
    })?;
    check_entrypoint(&so_path, read_manifest().as_ref(), Rebuild::Deploy)?;
    check_sbpf_version(&so_path, cluster_url)?;

    // Use the program's own keypair as --program-id so the deploy matches
    // `declare_id!` and upgrades in place instead of a new random address.
    let program_keypair = so_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|stem| so_path.with_file_name(format!("{stem}-keypair.json")))
        .filter(|p| p.exists());

    let mut deploy_cmd = Command::new("solana");
    deploy_cmd
        .arg("program")
        .arg("deploy")
        .arg("--url")
        .arg(cluster_url)
        .arg("--keypair")
        .arg(&expand_tilde(wallet_path)?);
    if let Some(program_keypair) = &program_keypair {
        println!("   Program keypair: {}", program_keypair.display());
        deploy_cmd.arg("--program-id").arg(program_keypair);
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
    }

    Ok(())
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

/// Falls back from `Pinoc.toml` to `solana config get` when the file is
/// missing, so `pinoc deploy` works without it. Skips both lookups if the
/// caller already passed both flags.
fn resolve_defaults(cluster: Option<&str>, wallet: Option<&str>) -> Result<(String, String)> {
    if cluster.is_some() && wallet.is_some() {
        return Ok((String::new(), String::new()));
    }

    if let Some(config) = config::read_pinoc_config_optional()? {
        return Ok((config.provider.cluster, config.provider.wallet));
    }

    println!("   Falling back to `solana config get` for cluster/wallet defaults.");
    read_solana_cli_config()
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
