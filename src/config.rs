use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct PinocConfig {
    pub provider: ProviderConfig,
    #[serde(default)]
    pub idl: IdlConfig,
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

/// Bails if `Pinoc.toml` is missing. Used by commands that genuinely require it (`deploy`).
pub fn read_pinoc_config() -> Result<PinocConfig> {
    let config_path = Path::new("Pinoc.toml");
    if !config_path.exists() {
        anyhow::bail!("Pinoc.toml not found. Please run this command from a project root.");
    }
    parse_pinoc_config(config_path)
}

/// Returns `None` if `Pinoc.toml` is missing, rather than bailing. Used by IDL
/// generation, which should fall through to auto-detect outside a full project.
pub fn read_pinoc_config_optional() -> Result<Option<PinocConfig>> {
    let config_path = Path::new("Pinoc.toml");
    if !config_path.exists() {
        return Ok(None);
    }
    Ok(Some(parse_pinoc_config(config_path)?))
}

fn parse_pinoc_config(config_path: &Path) -> Result<PinocConfig> {
    let config_content =
        fs::read_to_string(config_path).with_context(|| "Failed to read Pinoc.toml")?;
    toml::from_str(&config_content).with_context(|| "Failed to parse Pinoc.toml")
}
