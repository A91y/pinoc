//! `Pinoc.toml` schema, shared by `deploy` (`[provider]`), IDL generation
//! (`[idl]`), and client generation (`[client]`); not deploy-specific despite
//! the name.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct PinocConfig {
    pub provider: ProviderConfig,
    #[serde(default)]
    pub idl: IdlConfig,
    #[serde(default)]
    pub client: ClientConfig,
    #[serde(default)]
    pub check: CheckConfig,
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

#[derive(Debug, Default, Deserialize)]
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
}

/// Returns `None` (never errors) if `Pinoc.toml` is missing, so callers can
/// fall back to their own defaults; also prints a `pinoc config init` hint.
pub fn read_pinoc_config_optional() -> Result<Option<PinocConfig>> {
    let config_path = Path::new("Pinoc.toml");
    if !config_path.exists() {
        println!("💡 No Pinoc.toml found. Run `pinoc config init` to create one for this project.");
        return Ok(None);
    }
    Ok(Some(parse_pinoc_config(config_path)?))
}

fn parse_pinoc_config(config_path: &Path) -> Result<PinocConfig> {
    let config_content =
        fs::read_to_string(config_path).with_context(|| "Failed to read Pinoc.toml")?;
    toml::from_str(&config_content).with_context(|| "Failed to parse Pinoc.toml")
}
