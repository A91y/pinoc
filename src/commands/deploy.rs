use std::process::Command;

use anyhow::Context;

use crate::{expand_tilde, read_pinoc_config};

pub fn deploy(cluster: &Option<String>, wallet: &Option<String>) -> Result<(), anyhow::Error> {
    println!("Deploying program");

    let config = read_pinoc_config()?;

    let cluster_url = cluster.as_deref().unwrap_or(&config.provider.cluster);
    let wallet_path = wallet.as_deref().unwrap_or(&config.provider.wallet);

    println!("📋 Using configuration:");
    println!("   Cluster: {}", cluster_url);
    println!("   Wallet: {}", wallet_path);

    let target_deploy_dir = std::path::Path::new("target/deploy");
    if !target_deploy_dir.exists() {
        anyhow::bail!("target/deploy directory not found. Please run 'pinoc build' first.");
    }

    let mut so_file = None;
    for entry in std::fs::read_dir(target_deploy_dir)? {
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
        Ok(())
    }
}
