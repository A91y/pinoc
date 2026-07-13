use crate::templates;
use anyhow::{Context, Result};
use std::io::{IsTerminal, Write};
use std::path::Path;

#[derive(clap::Subcommand)]
pub enum ConfigCommands {
    /// Create a Pinoc.toml in the current project, if one doesn't already exist.
    Init {
        #[arg(short = 'y', long = "yes", help = "Skip the confirmation prompt when this doesn't look like a Pinocchio project")]
        yes: bool,
    },
}

pub fn init_config(yes: bool) -> Result<()> {
    let cargo_toml = Path::new("Cargo.toml");
    if !cargo_toml.exists() {
        anyhow::bail!("Cargo.toml not found. Please run this command from the project root.");
    }

    let config_path = Path::new("Pinoc.toml");
    if config_path.exists() {
        println!("✅ Pinoc.toml already exists, nothing to do.");
        return Ok(());
    }

    if !yes && !is_pinocchio_project(cargo_toml)? {
        confirm_not_pinocchio()?;
    }

    std::fs::write(config_path, templates::pinoc_toml())?;
    println!("✅ Created Pinoc.toml");
    Ok(())
}

fn is_pinocchio_project(cargo_toml: &Path) -> Result<bool> {
    let content = std::fs::read_to_string(cargo_toml)
        .with_context(|| format!("Failed to read {}", cargo_toml.display()))?;
    let manifest: toml::Value = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", cargo_toml.display()))?;
    Ok(manifest
        .get("dependencies")
        .and_then(|deps| deps.get("pinocchio"))
        .is_some())
}

fn confirm_not_pinocchio() -> Result<()> {
    if !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "This doesn't look like a Pinocchio project (no `pinocchio` dependency in Cargo.toml). \
             Pass -y/--yes to create Pinoc.toml anyway."
        );
    }
    print!("This doesn't look like a Pinocchio project (no `pinocchio` dependency in Cargo.toml). Create Pinoc.toml anyway? [y/N]: ");
    let _ = std::io::stdout().flush();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
        anyhow::bail!("Aborted.");
    }
    Ok(())
}
