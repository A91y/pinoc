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

/// Returns `None` if `Pinoc.toml` is missing, rather than erroring — callers
/// fall back to their own defaults (`solana config get` for deploy,
/// auto-detection for IDL generation). Prints a one-line hint pointing at
/// `pinoc config init` so the fallback path isn't a dead end.
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
