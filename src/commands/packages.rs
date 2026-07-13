use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub fn add_package(package_name: &str) -> Result<()> {
    let cargo_toml_path = Path::new("Cargo.toml");
    if !cargo_toml_path.exists() {
        anyhow::bail!(
            "Cargo.toml not found. Please run this command from the project root directory."
        );
    }

    println!("📦 Adding package: {}", package_name);
    let status = Command::new("cargo")
        .arg("add")
        .arg(package_name)
        .spawn()?
        .wait()
        .with_context(|| format!("Failed to add package: {}", package_name))?;

    if !status.success() {
        anyhow::bail!(
            "Failed to add package '{}' with exit code: {:?}",
            package_name,
            status.code()
        );
    } else {
        println!("✅ Package '{}' added successfully!", package_name);
    }

    Ok(())
}

#[derive(Debug)]
struct SearchResult {
    name: String,
    description: String,
    version: String,
}

pub fn search_packages(query: Option<&str>) -> Result<()> {
    let search_term = match query {
        Some(q) => format!("pinocchio {}", q),
        None => "pinocchio".to_string(),
    };

    println!("🔍 Searching for packages matching '{}'...\n", search_term);

    let output = Command::new("cargo")
        .arg("search")
        .arg(&search_term)
        .arg("--limit")
        .arg("20")
        .output()
        .with_context(|| "Failed to run 'cargo search'. Make sure cargo is installed.")?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("cargo search failed: {}", error);
    }

    let search_results = String::from_utf8_lossy(&output.stdout);
    let packages = parse_cargo_search_output(&search_results)?;

    if packages.is_empty() {
        println!("No packages found matching '{}'.", search_term);
        println!("💡 Try a different search term or check https://crates.io for more packages.");
        return Ok(());
    }

    println!("📦 Found {} package(s):\n", packages.len());

    for package in packages {
        println!("🔹 {}", package.name);
        println!("   Description: {}", package.description);
        println!("   Version: {}", package.version);
        println!("   Install: pinoc add {}", package.name);
        println!();
    }

    Ok(())
}

/// Parses `cargo search`'s line format: `name = "version"    # description`.
fn parse_cargo_search_output(output: &str) -> Result<Vec<SearchResult>> {
    let mut packages = Vec::new();

    for line in output.lines() {
        if line.trim().is_empty() || line.starts_with("...") {
            continue;
        }

        if let Some(equals_pos) = line.find(" = ") {
            let name = line[..equals_pos].trim().to_string();
            let rest = &line[equals_pos + 3..];

            if let Some(quote_end) = rest[1..].find('"') {
                let version = rest[1..quote_end + 1].to_string();
                let description = if let Some(hash_pos) = rest.find(" # ") {
                    rest[hash_pos + 3..].trim().to_string()
                } else {
                    "No description available".to_string()
                };

                packages.push(SearchResult {
                    name,
                    description,
                    version,
                });
            }
        }
    }

    Ok(packages)
}
