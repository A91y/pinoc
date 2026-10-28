#![cfg(unix)]

mod common;

use common::{pinoc, stderr, stdout};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

const ENABLE_V3: &str = "5cC3foj77CWun58pC51ebHFUWavHWKarWyR5UUik7dnC";
const DISABLE_OLDER: &str = "B8JJXCy5amZyWG9r7EnUYLwzXSXTxG7GZ1qZ1qggo83g";

/// An empty project directory with a `bin/` holding stand-ins for `cargo` and
/// `solana` that append their arguments to `calls.log`.
fn project(pinoc_toml: Option<&str>) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "pinoc-build-test-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    if let Some(content) = pinoc_toml {
        std::fs::write(dir.join("Pinoc.toml"), content).unwrap();
    }
    script(&dir, "cargo", "echo \"cargo $*\" >> calls.log");
    dir
}

fn script(dir: &Path, name: &str, body: &str) {
    let path = dir.join("bin").join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A `solana` whose `feature status` reports the two SBPF gates as given.
fn solana(dir: &Path, v3: &str, older: &str) {
    script(
        dir,
        "solana",
        &format!(
            r#"echo "solana $*" >> calls.log
if [ "$1" = feature ]; then
  echo '{{"features":[{{"id":"{ENABLE_V3}","status":"{v3}"}},{{"id":"{DISABLE_OLDER}","status":"{older}"}}]}}'
fi"#
        ),
    );
}

/// A minimal ELF64 header plus one executable section holding the entry address.
fn elf(sbpf_version: u32) -> Vec<u8> {
    let mut elf = vec![0u8; 0x40 + 0x40];
    elf[..6].copy_from_slice(b"\x7fELF\x02\x01");
    elf[0x18..0x20].copy_from_slice(&0x100u64.to_le_bytes());
    elf[0x28..0x30].copy_from_slice(&0x40u64.to_le_bytes());
    elf[0x30..0x34].copy_from_slice(&sbpf_version.to_le_bytes());
    elf[0x3a..0x3c].copy_from_slice(&0x40u16.to_le_bytes());
    elf[0x3c..0x3e].copy_from_slice(&1u16.to_le_bytes());
    elf[0x48..0x50].copy_from_slice(&0x4u64.to_le_bytes());
    elf[0x50..0x58].copy_from_slice(&0x100u64.to_le_bytes());
    elf[0x60..0x68].copy_from_slice(&0x10u64.to_le_bytes());
    elf
}

fn artifact(dir: &Path, sbpf_version: u32) {
    std::fs::create_dir_all(dir.join("target/deploy")).unwrap();
    std::fs::write(dir.join("target/deploy/prog.so"), elf(sbpf_version)).unwrap();
    // Most deploy tests are about something else, so the program has an identity.
    if !dir.join("src/lib.rs").exists() {
        declare(dir, 7);
        keypair(dir, "target/deploy/prog-keypair.json", 7);
    }
}

/// The address of a keypair whose public half is 32 copies of `byte`.
fn address(byte: u8) -> String {
    bs58::encode([byte; 32]).into_string()
}

/// A Solana CLI keypair file at `rel` holding `address(byte)`.
fn keypair(dir: &Path, rel: &str, byte: u8) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut bytes = vec![1u8; 32];
    bytes.extend([byte; 32]);
    std::fs::write(path, serde_json::to_string(&bytes).unwrap()).unwrap();
}

/// `src/lib.rs` declaring `address(byte)` as the program id.
fn declare(dir: &Path, byte: u8) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("src/lib.rs"),
        format!("pinocchio::address::declare_id!(\"{}\");\n", address(byte)),
    )
    .unwrap();
}

fn deploy_call(dir: &Path) -> Option<String> {
    calls(dir)
        .into_iter()
        .find(|c| c.starts_with("solana program deploy"))
}

/// A project named `prog` whose stand-in `cargo build-sbf` produces an artifact.
fn buildable_project() -> PathBuf {
    let dir = project(None);
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"prog\"\n\n[features]\nno-entrypoint = []\ntest-default = [\"no-entrypoint\"]\ndevnet = []\n",
    )
    .unwrap();
    std::fs::write(dir.join("built.so"), elf(0)).unwrap();
    script(
        &dir,
        "cargo",
        "echo \"cargo $*\" >> calls.log\nif [ \"$1\" = build-sbf ]; then mkdir -p target/deploy && cp built.so target/deploy/prog.so; fi",
    );
    dir
}

fn run(dir: &Path, args: &[&str]) -> Output {
    let path = format!(
        "{}:{}",
        dir.join("bin").display(),
        std::env::var("PATH").unwrap()
    );
    Command::new(env!("CARGO_BIN_EXE_pinoc"))
        .args(args)
        .current_dir(dir)
        .env("PATH", path)
        .output()
        .expect("failed to run pinoc")
}

fn calls(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("calls.log"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

const PROVIDER: &str = "[provider]\ncluster = \"localhost\"\nwallet = \"/tmp/id.json\"\n";

#[test]
fn build_passes_no_arch_unless_one_is_asked_for() {
    let dir = project(None);
    assert!(run(&dir, &["build"]).status.success());
    assert_eq!(calls(&dir), ["cargo build-sbf"]);
}

#[test]
fn the_arch_flag_reaches_the_sbf_build_of_build_and_test() {
    let dir = project(None);
    assert!(run(&dir, &["build", "--arch", "v3", "-F", "devnet"])
        .status
        .success());
    assert!(run(&dir, &["test", "--arch", "v3"]).status.success());
    assert_eq!(
        calls(&dir),
        [
            "cargo build-sbf --arch v3 --features devnet",
            "cargo build-sbf --arch v3",
            "cargo test",
        ]
    );
}

#[test]
fn the_arch_in_pinoc_toml_applies_and_the_flag_overrides_it() {
    let dir = project(Some(&format!("{PROVIDER}[build]\narch = \"v3\"\n")));
    assert!(run(&dir, &["build"]).status.success());
    assert!(run(&dir, &["test"]).status.success());
    assert!(run(&dir, &["build", "--arch", "v0"]).status.success());
    assert_eq!(
        calls(&dir),
        [
            "cargo build-sbf --arch v3",
            "cargo build-sbf --arch v3",
            "cargo test",
            "cargo build-sbf --arch v0",
        ]
    );
}

#[test]
fn an_arch_that_is_not_an_sbpf_version_is_refused() {
    let dir = project(Some(&format!("{PROVIDER}[build]\narch = \"v5\"\n")));
    let out = run(&dir, &["build"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("v5"), "{}", stderr(&out));
    assert!(calls(&dir).is_empty());

    let dir = project(Some(&format!("{PROVIDER}[build]\narhc = \"v3\"\n")));
    let out = run(&dir, &["build"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("arhc"), "{}", stderr(&out));

    let dir = project(None);
    assert!(!pinoc(&dir, &["build", "--arch", "v5"]).status.success());
}

#[test]
fn deploy_refuses_an_artifact_the_cluster_no_longer_deploys() {
    let dir = project(Some(PROVIDER));
    artifact(&dir, 0);
    solana(&dir, "active", "active");
    let out = run(&dir, &["deploy"]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains("target/deploy/prog.so is SBPFv0")
            && err.contains("SIMD-0500")
            && err.contains("pinoc build --arch v3"),
        "{err}"
    );
    assert!(!calls(&dir).iter().any(|c| c.contains("program deploy")));
}

#[test]
fn deploy_refuses_a_v3_artifact_where_v3_is_not_enabled() {
    let dir = project(Some(PROVIDER));
    artifact(&dir, 3);
    solana(&dir, "inactive", "inactive");
    let out = run(&dir, &["deploy"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("has not enabled SBPFv3"));
    assert!(!calls(&dir).iter().any(|c| c.contains("program deploy")));
}

#[test]
fn deploy_goes_ahead_when_the_cluster_takes_the_artifact() {
    for (version, v3, older) in [
        (0, "active", "inactive"),
        (3, "active", "inactive"),
        (3, "active", "active"),
    ] {
        let dir = project(Some(PROVIDER));
        artifact(&dir, version);
        solana(&dir, v3, older);
        let out = run(&dir, &["deploy"]);
        assert!(out.status.success(), "{}", stderr(&out));
        assert!(stdout(&out).contains("Program deployed successfully!"));
        assert!(calls(&dir).iter().any(|c| c.contains("program deploy")));
    }
}

#[test]
fn deploy_goes_ahead_when_the_cluster_cannot_be_asked() {
    let dir = project(Some(PROVIDER));
    artifact(&dir, 0);
    script(
        &dir,
        "solana",
        "echo \"solana $*\" >> calls.log\nif [ \"$1\" = feature ]; then exit 1; fi",
    );
    assert!(run(&dir, &["deploy"]).status.success());
    assert!(calls(&dir).iter().any(|c| c.contains("program deploy")));
}

#[test]
fn arguments_after_the_separator_reach_cargo_test() {
    let dir = project(None);
    assert!(run(&dir, &["test", "--no-build"]).status.success());
    assert!(run(
        &dir,
        &[
            "test",
            "--no-build",
            "-F",
            "devnet",
            "--",
            "--test",
            "client",
            "parses"
        ]
    )
    .status
    .success());
    assert!(run(
        &dir,
        &["test", "--no-build", "-q", "--", "--test", "client"]
    )
    .status
    .success());
    assert!(run(
        &dir,
        &["test", "--no-build", "-q", "--", "parses", "--", "--exact"]
    )
    .status
    .success());
    assert_eq!(
        calls(&dir),
        [
            "cargo test",
            "cargo test --features devnet --test client parses",
            "cargo test --test client -- --quiet",
            "cargo test parses -- --exact --quiet",
        ]
    );
}

#[test]
fn no_build_refuses_an_artifact_built_with_other_features() {
    let dir = buildable_project();
    assert!(run(&dir, &["build", "-F", "devnet"]).status.success());
    let record = common::read_json(&dir.join("target/deploy/prog.build.json"));
    assert_eq!(record["features"], serde_json::json!(["devnet"]));
    assert_eq!(record["arch"], serde_json::Value::Null);

    let out = run(&dir, &["test", "-F", "test-default", "--no-build"]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains("target/deploy/prog.so was built with --features devnet, but this run would build it with no features")
            && err.contains("Drop --no-build")
            && err.contains("--build-features \"devnet\""),
        "{err}"
    );
    assert!(!calls(&dir).iter().any(|c| c.starts_with("cargo test")));

    for args in [
        &["test", "-F", "test-default,devnet", "--no-build"][..],
        &[
            "test",
            "-F",
            "test-default",
            "--no-build",
            "--build-features",
            "devnet",
        ],
    ] {
        let out = run(&dir, args);
        assert!(out.status.success(), "{}", stderr(&out));
        assert!(!stdout(&out).contains("not built by pinoc"));
    }
}

#[test]
fn no_build_refuses_an_artifact_built_for_another_arch() {
    let dir = buildable_project();
    assert!(run(&dir, &["test", "--arch", "v3"]).status.success());
    let out = run(&dir, &["test", "--no-build"]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("was built with no features and --arch v3, but this run would build it with no features."),
        "{}",
        stderr(&out)
    );
    assert!(run(&dir, &["test", "--no-build", "--arch", "v3"])
        .status
        .success());
}

#[test]
fn no_build_says_when_it_cannot_tell_what_the_artifact_is() {
    let dir = buildable_project();
    artifact(&dir, 0);
    let out = run(&dir, &["test", "-F", "test-default", "--no-build"]);
    assert!(out.status.success());
    assert!(stdout(&out).contains("target/deploy/prog.so was not built by pinoc"));

    assert!(run(&dir, &["build", "-F", "devnet"]).status.success());
    artifact(&dir, 3);
    let out = run(&dir, &["test", "-F", "test-default", "--no-build"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("or has been rebuilt since"));
}

#[test]
fn deploy_refuses_a_keypair_that_is_not_the_declared_program() {
    // A fresh checkout: `cargo build-sbf` wrote a new keypair beside the artifact.
    let dir = project(Some(PROVIDER));
    declare(&dir, 7);
    keypair(&dir, "target/deploy/prog-keypair.json", 9);
    artifact(&dir, 0);
    solana(&dir, "active", "inactive");
    let out = run(&dir, &["deploy"]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains(&format!(
            "target/deploy/prog-keypair.json holds {}, but src/lib.rs declares {}",
            address(9),
            address(7)
        )) && err.contains("[provider] program_keypair")
            && err.contains("pinoc keys sync"),
        "{err}"
    );
    assert_eq!(deploy_call(&dir), None);
}

#[test]
fn deploy_uses_the_configured_program_keypair() {
    let dir = project(Some(&format!(
        "{PROVIDER}program_keypair = \"keys/program.json\"\nupgrade_authority = \"keys/admin.json\"\n"
    )));
    declare(&dir, 7);
    keypair(&dir, "target/deploy/prog-keypair.json", 9);
    keypair(&dir, "keys/program.json", 7);
    artifact(&dir, 0);
    solana(&dir, "active", "inactive");
    let out = run(&dir, &["deploy"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        deploy_call(&dir).unwrap(),
        "solana program deploy --url localhost --keypair /tmp/id.json --program-id keys/program.json --upgrade-authority keys/admin.json target/deploy/prog.so"
    );

    // The flag beats the file, and is held to the same check.
    let out = run(
        &dir,
        &[
            "deploy",
            "--program-keypair",
            "target/deploy/prog-keypair.json",
        ],
    );
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("--program-keypair"),
        "{}",
        stderr(&out)
    );

    let out = run(&dir, &["deploy", "--program-keypair", "keys/missing.json"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("keys/missing.json (from --program-keypair) does not exist"));
}

#[test]
fn deploy_without_a_keypair_upgrades_the_declared_address() {
    let dir = project(Some(PROVIDER));
    declare(&dir, 7);
    artifact(&dir, 0);
    solana(&dir, "active", "inactive");
    let out = run(&dir, &["deploy"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("A first deploy needs its keypair"));
    assert!(deploy_call(&dir)
        .unwrap()
        .contains(&format!("--program-id {} ", address(7))));
}

#[test]
fn deploy_refuses_when_nothing_names_the_address() {
    let dir = project(Some(PROVIDER));
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("src/lib.rs"), "\n").unwrap();
    artifact(&dir, 0);
    solana(&dir, "active", "inactive");
    let out = run(&dir, &["deploy"]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("new random address"),
        "{}",
        stderr(&out)
    );
    assert_eq!(deploy_call(&dir), None);
}

#[test]
fn deploy_picks_the_package_artifact_among_several() {
    let dir = buildable_project();
    std::fs::write(
        dir.join("Pinoc.toml"),
        format!("{PROVIDER}program_keypair = \"keys/program.json\"\n"),
    )
    .unwrap();
    declare(&dir, 7);
    keypair(&dir, "keys/program.json", 7);
    assert!(run(&dir, &["build"]).status.success());
    std::fs::write(dir.join("target/deploy/aaa_other.so"), elf(0)).unwrap();
    solana(&dir, "active", "inactive");
    let out = run(&dir, &["deploy"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(deploy_call(&dir)
        .unwrap()
        .ends_with("target/deploy/prog.so"));
}

const CLUSTERS: &str = "\n[clusters.mainnet]\nurl = \"mainnet-beta\"\n\n[clusters.devnet]\nfeatures = [\"devnet\"]\n\n[clusters.localnet]\nurl = \"localhost\"\nfeatures = [\"devnet\"]\nwallet = \"/tmp/local.json\"\n";

fn cluster_project() -> PathBuf {
    let dir = buildable_project();
    std::fs::write(
        dir.join("Pinoc.toml"),
        format!("{PROVIDER}program_keypair = \"keys/program.json\"\n{CLUSTERS}"),
    )
    .unwrap();
    declare(&dir, 7);
    keypair(&dir, "keys/program.json", 7);
    solana(&dir, "active", "inactive");
    dir
}

#[test]
fn a_cluster_build_adds_its_features_and_is_recorded() {
    let dir = cluster_project();
    assert!(run(&dir, &["build", "--cluster", "devnet", "-F", "extra"])
        .status
        .success());
    assert_eq!(calls(&dir), ["cargo build-sbf --features devnet,extra"]);
    let record = common::read_json(&dir.join("target/deploy/prog.build.json"));
    assert_eq!(record["features"], serde_json::json!(["devnet", "extra"]));
    assert_eq!(record["cluster"], "devnet");
    assert_eq!(record["sha256"].as_str().unwrap().len(), 64);

    let out = run(&dir, &["build", "--cluster", "testnet"]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("no `[clusters.testnet]`")
            && stderr(&out).contains("`devnet`, `localnet`, `mainnet`"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn deploy_refuses_an_artifact_built_for_another_cluster() {
    let dir = cluster_project();
    assert!(run(&dir, &["build", "--cluster", "devnet"])
        .status
        .success());

    let out = run(&dir, &["deploy", "--cluster", "mainnet"]);
    assert!(!out.status.success());
    let err = stderr(&out);
    assert!(
        err.contains("target/deploy/prog.so was built with --features devnet (for cluster `devnet`), which is not a build for cluster `mainnet`")
            && err.contains("`devnet` is a feature of cluster `devnet`")
            && err.contains("pinoc build --cluster mainnet"),
        "{err}"
    );
    assert_eq!(deploy_call(&dir), None);

    assert!(run(&dir, &["build"]).status.success());
    let out = run(&dir, &["deploy", "--cluster", "devnet"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("`devnet` builds with `devnet`"));
}

#[test]
fn deploy_to_a_cluster_uses_its_url_and_wallet() {
    let dir = cluster_project();
    assert!(run(&dir, &["build", "--cluster", "devnet"])
        .status
        .success());
    let out = run(&dir, &["deploy", "--cluster", "devnet"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(deploy_call(&dir)
        .unwrap()
        .starts_with("solana program deploy --url devnet --keypair /tmp/id.json"));

    // `[provider] cluster = "localhost"` is the localnet entry's url, so a bare
    // deploy is checked as localnet, which builds with devnet's features.
    let out = run(&dir, &["deploy"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("`[clusters.localnet]`"));
    assert!(deploy_call(&dir).is_some());
    assert!(calls(&dir)
        .iter()
        .any(|c| c.contains("--url localhost --keypair /tmp/local.json")));
}

/// A `solana` that answers `program dump` with the contents of `onchain.so`.
fn dumping_solana(dir: &Path) {
    script(
        dir,
        "solana",
        r#"echo "solana $*" >> calls.log
if [ "$1" = program ] && [ "$2" = dump ]; then cp onchain.so "$4"; fi"#,
    );
}

#[test]
fn verify_compares_the_deployed_program_with_the_artifact() {
    let dir = cluster_project();
    assert!(run(&dir, &["build", "--cluster", "devnet"])
        .status
        .success());
    dumping_solana(&dir);
    let local = std::fs::read(dir.join("target/deploy/prog.so")).unwrap();

    // Program accounts are often larger than the program; the rest is zero.
    let mut padded = local.clone();
    padded.extend([0u8; 64]);
    std::fs::write(dir.join("onchain.so"), &padded).unwrap();
    let out = run(&dir, &["verify", "--cluster", "devnet"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("The deployed program is target/deploy/prog.so")
            && text.contains("built by pinoc with --features devnet, for cluster `devnet`"),
        "{text}"
    );
    assert!(calls(&dir).iter().any(|c| c
        .starts_with(&format!("solana program dump {}", address(7)))
        && c.ends_with("--url devnet")));

    let mut changed = local.clone();
    changed[0x70] ^= 1;
    let mut padded = changed.clone();
    padded.extend([0u8; 64]);
    std::fs::write(dir.join("onchain.so"), &padded).unwrap();
    let out = run(&dir, &["verify", "--cluster", "devnet"]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains(&format!(
            "The program at {} is not target/deploy/prog.so",
            address(7)
        )),
        "{}",
        stderr(&out)
    );
    // The on-chain figure is the deployed program's own, whatever the local file.
    let text = stdout(&out);
    assert!(
        text.contains(&format!(
            "on chain {}  {} bytes (the program account holds {};",
            sha256_hex(&changed),
            changed.len(),
            padded.len()
        )) && text.contains("built by pinoc with --features devnet, for cluster `devnet`"),
        "{text}"
    );
}

#[test]
fn verify_refuses_an_artifact_built_for_another_cluster() {
    let dir = cluster_project();
    assert!(run(&dir, &["build", "--cluster", "mainnet"])
        .status
        .success());
    dumping_solana(&dir);
    let out = run(&dir, &["verify", "--cluster", "devnet"]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("target/deploy/prog.so was built with no features (for cluster `mainnet`), which is not a build for cluster `devnet`")
            && stderr(&out).contains("pinoc build --cluster devnet"),
        "{}",
        stderr(&out)
    );
    assert!(!calls(&dir).iter().any(|c| c.contains("program dump")));
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
fn test_adds_the_configured_features_and_builds_the_test_programs() {
    let dir = buildable_project();
    std::fs::create_dir_all(dir.join("tests/stub")).unwrap();
    std::fs::write(
        dir.join("tests/stub/Cargo.toml"),
        "[package]\nname = \"stub\"\n",
    )
    .unwrap();
    // The stand-in cargo logs where it ran, so the stub's build is told apart.
    script(
        &dir,
        "cargo",
        &format!(
            "echo \"cargo $* @${{PWD##*/}}\" >> {}\nif [ \"$1\" = build-sbf ] && [ -f built.so ]; then mkdir -p target/deploy && cp built.so target/deploy/prog.so; fi",
            dir.join("calls.log").display()
        ),
    );
    std::fs::write(
        dir.join("Pinoc.toml"),
        format!("{PROVIDER}[build]\narch = \"v3\"\n\n[test]\nfeatures = [\"test-default\"]\nprograms = [\"tests/stub\"]\n{CLUSTERS}"),
    )
    .unwrap();
    let out = run(&dir, &["test", "--cluster", "devnet"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let name = dir.file_name().unwrap().to_str().unwrap().to_string();
    assert_eq!(
        calls(&dir),
        [
            format!("cargo build-sbf --arch v3 --features devnet @{name}"),
            "cargo build-sbf --arch v3 @stub".to_string(),
            format!("cargo test --features test-default,devnet @{name}"),
        ]
    );

    std::fs::write(
        dir.join("Pinoc.toml"),
        format!("{PROVIDER}[test]\nprograms = [\"tests/nope\"]\n"),
    )
    .unwrap();
    let out = run(&dir, &["test"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("\"tests/nope\", which has no Cargo.toml"));
}

#[test]
fn all_clusters_runs_once_per_feature_set() {
    let dir = cluster_project();
    std::fs::write(
        dir.join("Pinoc.toml"),
        format!("{PROVIDER}[test]\nfeatures = [\"test-default\"]\n{CLUSTERS}"),
    )
    .unwrap();
    let out = run(&dir, &["test", "--all-clusters"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("== Testing cluster `devnet`, `localnet`"));
    assert_eq!(
        calls(&dir),
        [
            "cargo build-sbf --features devnet",
            "cargo test --features test-default,devnet",
            "cargo build-sbf",
            "cargo test --features test-default",
        ]
    );

    // A failing run does not stop the others, and the command fails.
    script(
        &dir,
        "cargo",
        "echo \"cargo $*\" >> calls.log\nif [ \"$1\" = build-sbf ]; then mkdir -p target/deploy && cp built.so target/deploy/prog.so; fi\ncase \"$*\" in *devnet*) [ \"$1\" = test ] && exit 101;; esac\nexit 0",
    );
    std::fs::remove_file(dir.join("calls.log")).unwrap();
    let out = run(&dir, &["test", "--all-clusters"]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("Tests failed for cluster `devnet`, `localnet` (1 of 2 runs)"),
        "{}",
        stderr(&out)
    );
    assert_eq!(calls(&dir).len(), 4);

    let dir = project(Some(PROVIDER));
    let out = run(&dir, &["test", "--all-clusters"]);
    assert!(!out.status.success());
    assert!(stderr(&out).contains("declares none"));
}
