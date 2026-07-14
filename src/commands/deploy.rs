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
