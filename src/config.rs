//! `Pinoc.toml` schema, shared by `deploy` (`[provider]`, `[clusters]`), the
//! SBF build (`[build]`, `[clusters]`), `test` (`[test]`), IDL generation
//! (`[idl]`), and client generation (`[client]`); not deploy-specific despite
//! the name.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

// An unknown section is refused: a misspelt `[check]` would otherwise be a
// config that silently does nothing.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinocConfig {
    pub provider: ProviderConfig,
    #[serde(default)]
    pub idl: IdlConfig,
    #[serde(default)]
    pub client: ClientConfig,
    #[serde(default)]
    pub check: CheckConfig,
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub test: TestConfig,
    /// Named clusters, each with the features its build needs.
    #[serde(default)]
    pub clusters: BTreeMap<String, ClusterConfig>,
}

/// The SBPF version `cargo build-sbf --arch` builds for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Arch {
    V0,
    V1,
    V2,
    V3,
    V4,
}

impl Arch {
    pub fn as_str(self) -> &'static str {
        match self {
            Arch::V0 => "v0",
            Arch::V1 => "v1",
            Arch::V2 => "v2",
            Arch::V3 => "v3",
            Arch::V4 => "v4",
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildConfig {
    /// Passed to `cargo build-sbf --arch` by `pinoc build` and `pinoc test`.
    /// Absent means the flag is not passed and the toolchain's default applies.
    #[serde(default)]
    pub arch: Option<Arch>,
}

#[derive(Debug, Deserialize)]
pub struct ProviderConfig {
    pub cluster: String,
    pub wallet: String,
    /// The program's own keypair, used as `--program-id` by `pinoc deploy`.
    /// Absent means `target/deploy/<name>-keypair.json`.
    #[serde(default)]
    pub program_keypair: Option<String>,
    /// Passed to `solana program deploy --upgrade-authority`.
    #[serde(default)]
    pub upgrade_authority: Option<String>,
}

/// One `[clusters.<name>]` entry. `pinoc build --cluster <name>` builds with its
/// features, and `pinoc deploy --cluster <name>` refuses an artifact that was
/// built for another cluster.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClusterConfig {
    /// What `solana --url` is given. Absent means the cluster's own name, so
    /// `[clusters.devnet]` reaches devnet with no `url` at all.
    #[serde(default)]
    pub url: Option<String>,
    /// Overrides `[provider] wallet` for this cluster.
    #[serde(default)]
    pub wallet: Option<String>,
    /// Overrides `[provider] upgrade_authority` for this cluster.
    #[serde(default)]
    pub upgrade_authority: Option<String>,
    /// The cargo features an artifact for this cluster is built with.
    #[serde(default)]
    pub features: Vec<String>,
}

/// What `pinoc test` adds to every run.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestConfig {
    /// Features every `cargo test` runs with, before those given with `--features`.
    #[serde(default)]
    pub features: Vec<String>,
    /// Other SBF programs the tests load (a stand-in for a program the one
    /// under test calls), each a directory holding its own `Cargo.toml`. Built
    /// with `cargo build-sbf` and the same `--arch`, before the tests run.
    #[serde(default)]
    pub programs: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct IdlConfig {
    /// "auto" | "shank" | "codama"; absent or "auto" means auto-detect.
    #[serde(default)]
    pub generator: Option<String>,
    /// Where `pinoc build` and `pinoc idl` write the IDL, and where `pinoc
    /// client generate` reads it. Absent means `target/idl`.
    #[serde(default)]
    pub out_dir: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ClientConfig {
    #[serde(default)]
    pub out_dir: Option<String>,
    #[serde(default)]
    pub shank_out_dir: Option<String>,
    #[serde(default)]
    pub codama_out_dir: Option<String>,
    /// Where the TypeScript client goes. Absent means `clients/ts`.
    #[serde(default)]
    pub ts_out_dir: Option<String>,
    /// The clients `pinoc client generate` renders when `--language` is not given.
    #[serde(default)]
    pub languages: Vec<crate::client_gen::Language>,
    /// The Rust client's generator when `--generator` is not given.
    #[serde(default)]
    pub generator: Option<crate::idl::Generator>,
}

// An unknown key is refused: a misspelt `authority_names` would otherwise
// leave a finding unreported, which reads the same as a clean program.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckConfig {
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default)]
    pub warn: Vec<String>,
    #[serde(default)]
    pub allow: Vec<String>,
    /// "heuristic" | "likely" | "definite"; findings weaker than this are dropped.
    #[serde(default)]
    pub confidence_threshold: Option<String>,
    /// Names, besides `authority`/`admin`/`auth`, that hold an authority's key
    /// in this program (`keeper`, `operator`). `ACC002-P` treats an account
    /// compared against one of them as an authority that must sign.
    #[serde(default)]
    pub authority_names: Vec<String>,
}

/// Returns `None` (never errors) if `Pinoc.toml` is missing, so callers can fall
/// back to their own defaults. The `pinoc config init` hint goes to stderr so it
/// never pollutes machine-readable stdout (e.g. `pinoc check --json`).
pub fn read_pinoc_config_optional() -> Result<Option<PinocConfig>> {
    let config_path = Path::new("Pinoc.toml");
    if !config_path.exists() {
        eprintln!(
            "💡 No Pinoc.toml found. Run `pinoc config init` to create one for this project."
        );
        return Ok(None);
    }
    Ok(Some(parse_pinoc_config(config_path)?))
}

/// `Pinoc.toml`, or `None` when it is absent, without the `pinoc config init` hint.
pub fn load() -> Result<Option<PinocConfig>> {
    let config_path = Path::new("Pinoc.toml");
    if !config_path.exists() {
        return Ok(None);
    }
    Ok(Some(parse_pinoc_config(config_path)?))
}

/// `[build].arch` from `Pinoc.toml`, or `None` when the file or the key is absent.
pub fn build_arch() -> Result<Option<Arch>> {
    Ok(load()?.and_then(|c| c.build.arch))
}

/// `[idl].out_dir`, else `target/idl`.
pub fn idl_out_dir() -> Result<String> {
    Ok(load()?
        .and_then(|c| c.idl.out_dir)
        .unwrap_or_else(|| "target/idl".to_string()))
}

/// `[clusters.<name>]`, with `name` in the error when there is no such entry.
pub fn cluster(name: &str) -> Result<(PinocConfig, ClusterConfig)> {
    let Some(config) = load()? else {
        anyhow::bail!(
            "--cluster {name} names a `[clusters.{name}]` entry, but there is no Pinoc.toml"
        );
    };
    match config.clusters.get(name).cloned() {
        Some(cluster) => Ok((config, cluster)),
        None => {
            let known = config
                .clusters
                .keys()
                .map(|k| format!("`{k}`"))
                .collect::<Vec<_>>();
            anyhow::bail!(
                "Pinoc.toml has no `[clusters.{name}]`. {}",
                if known.is_empty() {
                    "It declares no clusters.".to_string()
                } else {
                    format!("It declares {}.", known.join(", "))
                }
            )
        }
    }
}

fn parse_pinoc_config(config_path: &Path) -> Result<PinocConfig> {
    let config_content =
        fs::read_to_string(config_path).with_context(|| "Failed to read Pinoc.toml")?;
    toml::from_str(&config_content).with_context(|| "Failed to parse Pinoc.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scaffold_template_parses() {
        toml::from_str::<PinocConfig>(crate::templates::pinoc_toml()).unwrap();
    }

    #[test]
    fn every_section_parses_and_a_misspelt_key_is_refused() {
        let config: PinocConfig = toml::from_str(
            r#"
[provider]
cluster = "localhost"
wallet = "~/.config/solana/id.json"
program_keypair = "../keys/program.json"

[idl]
generator = "codama"
out_dir = "idl"

[client]
languages = ["ts", "rust"]
generator = "codama"

[build]
arch = "v3"

[test]
features = ["test-default"]
programs = ["tests/mock_venue"]

[clusters.devnet]
features = ["devnet"]

[clusters.localnet]
url = "localhost"
features = ["devnet"]
"#,
        )
        .unwrap();
        assert_eq!(
            config.clusters["localnet"].url.as_deref(),
            Some("localhost")
        );
        assert_eq!(config.test.programs, ["tests/mock_venue"]);
        assert_eq!(
            config.client.languages,
            [
                crate::client_gen::Language::Ts,
                crate::client_gen::Language::Rust
            ]
        );

        for bad in [
            "[clusters.devnet]\nfeature = [\"devnet\"]\n",
            "[test]\nprogram = [\"x\"]\n",
        ] {
            let err = toml::from_str::<PinocConfig>(&format!(
                "[provider]\ncluster = \"l\"\nwallet = \"w\"\n{bad}"
            ))
            .unwrap_err()
            .to_string();
            assert!(err.contains("unknown field"), "{err}");
        }
    }
}
