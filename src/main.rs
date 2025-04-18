use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::Path;
use std::process::Command;

// Import the content module
mod content;
use content::templates;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init { project_name: String },
    Build,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init { project_name } => {
            init_project(project_name)?;
        }
        Commands::Build => {
            println!("Building project");
            let status = Command::new("cargo")
                .arg("build-sbf")
                .spawn()?
                .wait()
                .with_context(|| "Failed to build project")?;

            if !status.success() {
                anyhow::bail!("Build failed with exit code: {:?}", status.code());
            } else {
                println!("Build completed successfully!");
            }
        }
    }

    Ok(())
}

fn init_project(project_name: &str) -> Result<()> {
    println!("\x1b[38;2;255;160;122m"); // Custom RGB color for coral/orange
    println!(
        r#"
▄▄▄▄· ▄▄▄  ▄• ▄▌ ▐ ▄       
▐█ ▀█▪▀▄ █·█▪██▌•█▌▐█▪     
▐█▀▀█▄▐▀▀▄ █▌▐█▌▐█▐▐▌ ▄█▀▄ 
██▄▪▐█▐█•█▌▐█▄█▌██▐█▌▐█▌.▐▌
·▀▀▀▀ .▀  ▀ ▀▀▀ ▀▀ █▪ ▀█▄▀▪
"#
    );
    println!("\x1b[0m");

    // Display project initialization message with styling
    println!(
        "\x1b[38;2;255;160;122m🚀 Initializing Chio Project: {}\x1b[0m",
        project_name
    );
    println!("\x1b[90m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");

    // Create the project directory
    let project_dir = Path::new(project_name);
    fs::create_dir_all(project_dir)
        .with_context(|| format!("Failed to create project directory: {}", project_name))?;

    // Create a new Cargo project inside
    let output = Command::new("cargo")
        .arg("init")
        .arg("--lib")
        .arg("--name")
        .arg(project_name)
        .current_dir(project_dir)
        .output()
        .with_context(|| "Failed to run 'cargo init'")?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to initialize Cargo project: {}", error);
    }

    //solana cli for address
    let address_output = Command::new("solana")
        .arg("address")
        .current_dir(project_dir)
        .output()
        .with_context(|| "Failed to run fetch address'")?;
    let address: String;
    if address_output.status.success() {
        address = String::from_utf8_lossy(&address_output.stdout)
            .trim()
            .to_string();
    } else {
        let error = String::from_utf8_lossy(&address_output.stderr);
        println!("Failed to get Solana address: {}", error);
        address = String::new();
    }
    // Use this user address in unit_tests
    println!("Solana address: {}", address);

    // create project structure and Cargo.toml
    create_project_structure(project_dir)?;
    update_cargo_toml(project_dir, project_name)?;

    // adding something to create keypair and put that address to program in lib.rs

    println!("\x1b[90m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
    println!(
        "\x1b[38;2;255;160;122m✅ Project '{}' initialized successfully!\x1b[0m",
        project_name
    );
    println!("\x1b[38;2;255;160;122m$ cd {}\x1b[0m", project_name);
    println!("\x1b[38;2;255;160;122m$ chio build\x1b[0m");
    println!("\x1b[38;2;255;160;122m$ chio deploy\x1b[0m");
    println!("\x1b[38;2;255;160;122m$ chio help\x1b[0m");

    println!("\x1b[90m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
    Ok(())
}

fn create_project_structure(project_dir: &Path) -> Result<()> {
    // Create configuration files root folder
    fs::write(project_dir.join("README.md"), templates::readme_md())?;
    fs::write(project_dir.join(".gitignore"), templates::gitignore())?;

    // Create src directory structure
    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    // Create lib.rs
    fs::write(src_dir.join("lib.rs"), templates::lib_rs())?;

    // Create entrypoint.rs
    fs::write(src_dir.join("entrypoint.rs"), templates::entrypoint_rs())?;

    // Create errors.rs
    fs::write(src_dir.join("errors.rs"), templates::errors_rs())?;

    //Creating instruction folder and .rs(s)
    let instructions_dir = src_dir.join("instructions");
    fs::create_dir_all(&instructions_dir)?;

    fs::write(
        instructions_dir.join("mod.rs"),
        templates::instructions::instructions_mod_rs(),
    )?;
    fs::write(
        instructions_dir.join("deposit.rs"),
        templates::instructions::deposit_rs(),
    )?;
    fs::write(
        instructions_dir.join("withdraw.rs"),
        templates::instructions::withdraw_rs(),
    )?;

    //Creating states folder and .rs(s)
    let states_dir = src_dir.join("states");
    fs::create_dir_all(&states_dir)?;

    fs::write(
        states_dir.join("mod.rs"),
        templates::states::states_mod_rs(),
    )?;
    fs::write(states_dir.join("utils.rs"), templates::states::utils_rs())?;

    Ok(())
}

fn update_cargo_toml(project_dir: &Path, project_name: &str) -> Result<()> {
    let cargo_toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
pinocchio = "0.8.1"
pinocchio-log = "0.4.0"
pinocchio-pubkey = "0.2.4"
pinocchio-system = "0.2.3"
pinocchio-token = "0.3.0"

[dev-dependencies]
solana-sdk = "2.1.0"
mollusk-svm = "0.1.4"
spl-token = "8.0.0"
mollusk-svm-bencher = "0.1.4"

[features]
no-entrypoint = []
std = []
test-default = ["no-entrypoint", "std"]
bench-default = ["no-entrypoint", "std"]

[[test]]
name = "unit_tests""#,
        project_name
    );

    fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;

    Ok(())
}
