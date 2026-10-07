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
