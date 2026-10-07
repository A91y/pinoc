//! `Pinoc.toml` schema, shared by `deploy` (`[provider]`), IDL generation
//! (`[idl]`), and client generation (`[client]`); not deploy-specific despite
//! the name.

use anyhow::{Context, Result};
use serde::Deserialize;
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
}

/// The SBPF version `cargo build-sbf --arch` builds for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, clap::ValueEnum)]
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
}

#[derive(Debug, Default, Deserialize)]
pub struct IdlConfig {
    /// "auto" | "shank" | "codama"; absent or "auto" means auto-detect.
    #[serde(default)]
    pub generator: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ClientConfig {
    #[serde(default)]
    pub out_dir: Option<String>,
    #[serde(default)]
    pub shank_out_dir: Option<String>,
    #[serde(default)]
    pub codama_out_dir: Option<String>,
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

/// `[build].arch` from `Pinoc.toml`, or `None` when the file or the key is absent.
pub fn build_arch() -> Result<Option<Arch>> {
    let config_path = Path::new("Pinoc.toml");
    if !config_path.exists() {
        return Ok(None);
    }
    Ok(parse_pinoc_config(config_path)?.build.arch)
}

fn parse_pinoc_config(config_path: &Path) -> Result<PinocConfig> {
    let config_content =
        fs::read_to_string(config_path).with_context(|| "Failed to read Pinoc.toml")?;
    toml::from_str(&config_content).with_context(|| "Failed to parse Pinoc.toml")
}
