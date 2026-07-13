use crate::config::read_pinoc_config;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn run_deploy(cluster: Option<&str>, wallet: Option<&str>) -> Result<()> {
    println!("Deploying program");

    let config = read_pinoc_config()?;

    let cluster_url = cluster.unwrap_or(&config.provider.cluster);
    let wallet_path = wallet.unwrap_or(&config.provider.wallet);

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

    let mut deploy_cmd = Command::new("solana");
    deploy_cmd
        .arg("program")
        .arg("deploy")
        .arg("--url")
        .arg(cluster_url)
        .arg("--keypair")
        .arg(&expand_tilde(wallet_path)?)
        .arg(&so_path);

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
