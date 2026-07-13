use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(clap::Subcommand)]
pub enum KeyCommands {
    List,
    Sync,
}

pub fn list_program_keys() -> Result<()> {
    println!("🔑 Listing program keys...");

    let deploy_dir = Path::new("target/deploy");
    if !deploy_dir.exists() {
        println!("❌ No target/deploy directory found. Run 'pinoc build' first.");
        return Ok(());
    }

    let mut found_keys = Vec::new();

    for entry in fs::read_dir(deploy_dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(file_name) = path.file_name() {
            if let Some(name_str) = file_name.to_str() {
                if name_str.ends_with("-keypair.json") {
                    let program_name = name_str.replace("-keypair.json", "");

                    let address_output = Command::new("solana")
                        .arg("address")
                        .arg("-k")
                        .arg(&path)
                        .output()
                        .with_context(|| format!("Failed to read keypair address: {}", name_str))?;

                    if address_output.status.success() {
                        let pubkey = String::from_utf8_lossy(&address_output.stdout)
                            .trim()
                            .to_string();
                        found_keys.push((program_name, pubkey, path));
                    }
                }
            }
        }
    }

    if found_keys.is_empty() {
        println!("❌ No program keypairs found in target/deploy/");
        println!("💡 Run 'pinoc build' to generate keypairs");
        return Ok(());
    }

    println!("\n📋 Program Keys:");
    println!("{:<20} {:<50} {:<30}", "Program", "Public Key", "Keypair File");
    println!("{:-<20} {:-<50} {:-<30}", "", "", "");

    for (program_name, pubkey, keypair_path) in found_keys {
        println!(
            "{:<20} {:<50} {}",
            program_name,
            pubkey,
            keypair_path.file_name().unwrap().to_str().unwrap()
        );
    }

    Ok(())
}

pub fn sync_program_keys() -> Result<()> {
    println!("🔄 Syncing program keys...");

    let cargo_toml = Path::new("Cargo.toml");
    if !cargo_toml.exists() {
        anyhow::bail!("Cargo.toml not found. Please run this command from a project root.");
    }

    let cargo_content =
        fs::read_to_string(cargo_toml).with_context(|| "Failed to read Cargo.toml")?;

    let project_name = extract_project_name(&cargo_content)
        .ok_or_else(|| anyhow::anyhow!("Could not find project name in Cargo.toml"))?;

    let keypair_path = format!("target/deploy/{}-keypair.json", project_name);
    let keypair_file = Path::new(&keypair_path);

    if !keypair_file.exists() {
        anyhow::bail!(
            "Keypair file not found: {}. Run 'pinoc build' first.",
            keypair_path
        );
    }

    let address_output = Command::new("solana")
        .arg("address")
        .arg("-k")
        .arg(&keypair_path)
        .output()
        .with_context(|| "Failed to read keypair address")?;

    if !address_output.status.success() {
        anyhow::bail!("Failed to get program address from keypair");
    }

    let actual_pubkey = String::from_utf8_lossy(&address_output.stdout)
        .trim()
        .to_string();

    let lib_rs_path = Path::new("src/lib.rs");
    if !lib_rs_path.exists() {
        anyhow::bail!("src/lib.rs not found");
    }

    let lib_content =
        fs::read_to_string(lib_rs_path).with_context(|| "Failed to read src/lib.rs")?;

    if let Some(current_pubkey) = extract_current_program_id(&lib_content) {
        if current_pubkey == actual_pubkey {
            println!("✅ Program key is already consistent!");
            println!("🔑 Program ID: {}", actual_pubkey);
            println!("📝 No update needed in src/lib.rs");
            return Ok(());
        } else {
            println!("🔄 Program key mismatch detected:");
            println!("   Current in lib.rs: {}", current_pubkey);
            println!("   Actual keypair:    {}", actual_pubkey);
        }
    }

    if let Some(updated_content) = update_declare_id(&lib_content, &actual_pubkey) {
        fs::write(lib_rs_path, updated_content)
            .with_context(|| "Failed to write updated src/lib.rs")?;

        println!("✅ Successfully synced program key!");
        println!("🔑 Program ID: {}", actual_pubkey);
        println!("📝 Updated src/lib.rs with new program ID");
    } else {
        println!("⚠️  No declare_id! macro found in src/lib.rs");
        println!("💡 Add this line to your lib.rs:");
        println!("   pinocchio_pubkey::declare_id!(\"{}\");", actual_pubkey);
    }

    Ok(())
}

fn extract_project_name(cargo_content: &str) -> Option<String> {
    for line in cargo_content.lines() {
        if line.trim().starts_with("name = ") {
            if let Some(name) = line.split('=').nth(1) {
                return Some(name.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

fn update_declare_id(lib_content: &str, new_pubkey: &str) -> Option<String> {
    let mut updated = false;
    let mut lines = Vec::new();

    for line in lib_content.lines() {
        if line.contains("declare_id!") {
            lines.push(format!(
                "pinocchio_pubkey::declare_id!(\"{}\");",
                new_pubkey
            ));
            updated = true;
        } else {
            lines.push(line.to_string());
        }
    }

    if updated {
        Some(lines.join("\n"))
    } else {
        None
    }
}

fn extract_current_program_id(lib_content: &str) -> Option<String> {
    for line in lib_content.lines() {
        if line.contains("declare_id!") {
            // matches both declare_id! and pinocchio_pubkey::declare_id!
            if let Some(start) = line.find("declare_id!(\"") {
                let after_declare = &line[start + 13..]; // Skip "declare_id!(\""
                if let Some(end) = after_declare.find("\"") {
                    return Some(after_declare[..end].to_string());
                }
            }
        }
    }
    None
}
