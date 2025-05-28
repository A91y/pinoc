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
    Init {
        project_name: String,
    },
    Build,
    #[command(name = "--help")]
    Help,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init { project_name } => {
            init_project(project_name)?;
        }
        Commands::Build => {
            println!("Building program");
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
        Commands::Build => {
            println!("Testing program");
            let status = Command::new("cargo")
                .arg("test")
                .spawn()?
                .wait()
                .with_context(|| "Failed to test project")?;

            if !status.success() {
                anyhow::bail!("Test failed with exit code: {:?}", status.code());
            } else {
                println!("Tested successfully!");
            }
        }
        Commands::Help => {
            display_help_banner()?;
        }
    }

    Ok(())
}

fn display_help_banner() -> Result<()> {
    // Display the banner
    println!("\x1b[38;2;255;175;193m");
    println!(
        r#"
      *     *       
  ___| |__ (_) ___  
 / __| '_ \| |/ _ \ 
| (__| | | | | (_) |
 \___|_| |_|_|\___/ 
 "#
    );
    println!("\x1b[0m");

    println!("👾 Setup your pinocchio project blazingly fast💨");

    println!("\n🏗️ AVAILABLE COMMANDS:");
    println!("   chio init <project_name> - Initialize a new Pinocchio project");
    println!("   chio build               - Build the project");
    println!("   chio test                - Run project tests");
    println!("   chio deploy              - Deploy the project");

    println!("\x1b[38;2;230;230;230m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");

    Ok(())
}

fn init_project(project_name: &str) -> Result<()> {
    println!("\x1b[38;2;255;175;193m"); // Custom RGB color for light pink (similar to the rabbit's ear)
    println!(
        r#"
      *     *       
  ___| |__ (_) ___  
 / __| '_ \| |/ _ \ 
| (__| | | | | (_) |
 \___|_| |_|_|\___/ 
                    
 "#
    );
    println!("\x1b[0m");
    // Display project initialization message with styling
    println!(
        "\x1b[38;2;255;175;193m🧑🏻‍🍳 Initializing your pinocchio project: {}\x1b[0m",
        project_name
    );
    println!("\x1b[38;2;230;230;230m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m"); // Create the project directory
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
    //println!("Solana address: {}", address);

    // create project structure and Cargo.toml
    create_project_structure(project_dir, address)?;
    update_cargo_toml(project_dir, project_name)?;

    // TODO: adding something to create keypair and put that address to program in lib.rs

    println!("\x1b[38;2;230;230;230m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
    println!(
        "\x1b[38;2;255;175;193m✅ Pinocchio Project '{}' initialized successfully!\x1b[0m",
        project_name
    );
    println!("\n\x1b[38;2;255;175;193m📋 Next steps:\x1b[0m");
    println!("\x1b[38;2;255;175;193m$ cd {}\x1b[0m", project_name);
    println!("\x1b[38;2;255;175;193m$ chio build\x1b[0m");
    println!("\x1b[38;2;255;175;193m$ chio test\x1b[0m");
    println!("\x1b[38;2;255;175;193m$ chio deploy\x1b[0m");
    println!("\x1b[38;2;230;230;230m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");

    Ok(())
}

fn create_project_structure(project_dir: &Path, address: String) -> Result<()> {
    // Create configuration files root folder
    fs::write(project_dir.join("README.md"), templates::readme_md())?;
    fs::write(project_dir.join(".gitignore"), templates::gitignore())?;

    // Create src directory structure
    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    // Create lib.rs
    // TODO: Pass program address
    let address = "Fg6PaFpoGXkYsidMpWxTWqMRMLuV7tQJjdtc1AGtX9pN";
    fs::write(src_dir.join("lib.rs"), templates::lib_rs(address))?;

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
        instructions_dir.join("initilaize.rs"),
        templates::instructions::initilaize(),
    )?;

    //Creating states folder and .rs(s)
    let states_dir = src_dir.join("states");
    fs::create_dir_all(&states_dir)?;

    fs::write(
        states_dir.join("mod.rs"),
        templates::states::states_mod_rs(),
    )?;
    fs::write(states_dir.join("utils.rs"), templates::states::utils_rs())?;

    fs::write(states_dir.join("state.rs"), templates::states::state_rs())?;

    //creating unit_tests folder
    let test_dir = project_dir.join("tests");
    fs::create_dir_all(&test_dir)?;

    fs::write(
        test_dir.join("unit_tests.rs"),
        templates::unit_tests::unit_test_rs(&address),
    )?;

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
