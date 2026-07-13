use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Retries removal since a just-exited cargo process can transiently hold the dir open.
fn remove_dir_all_with_retry(path: &Path) -> std::io::Result<()> {
    let mut last_err = None;
    for attempt in 0..5 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap())
}

pub fn clean_project(no_preserve: bool) -> Result<()> {
    println!("🧹 Cleaning project...");

    let target_dir = Path::new("target");
    if !target_dir.exists() {
        println!("✅ No target directory found. Nothing to clean.");
        return Ok(());
    }

    let deploy_dir = target_dir.join("deploy");
    let mut preserved_keypairs = Vec::new();

    if !no_preserve && deploy_dir.exists() {
        for entry in fs::read_dir(&deploy_dir)? {
            let entry = entry?;
            let path = entry.path();
            if let Some(file_name) = path.file_name() {
                if let Some(name_str) = file_name.to_str() {
                    if name_str.ends_with("-keypair.json") {
                        let keypair_name = name_str.to_string();
                        let keypair_content = fs::read(&path)
                            .with_context(|| format!("Failed to read keypair: {}", name_str))?;
                        preserved_keypairs.push((keypair_name, keypair_content));
                        println!("🔐 Preserving keypair: {}", name_str);
                    }
                }
            }
        }
    }

    remove_dir_all_with_retry(target_dir).with_context(|| "Failed to remove target directory")?;

    if !no_preserve {
        fs::create_dir_all(&deploy_dir)
            .with_context(|| "Failed to recreate target/deploy directory")?;

        let keypair_count = preserved_keypairs.len();
        for (keypair_name, keypair_content) in preserved_keypairs {
            let new_path = deploy_dir.join(&keypair_name);
            fs::write(&new_path, keypair_content)
                .with_context(|| format!("Failed to restore keypair: {}", keypair_name))?;
        }

        println!("✅ Project cleaned successfully!");
        if keypair_count > 0 {
            println!("🔐 Preserved {} keypair file(s)", keypair_count);
        } else {
            println!("✅ Project cleaned successfully!");
        }
    } else {
        println!("✅ Project cleaned successfully! (keypairs not preserved)");
    }

    Ok(())
}
