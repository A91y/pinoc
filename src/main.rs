use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::Path;
use std::process::Command;

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
    Test,
    Deploy,
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
        Commands::Test => {
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
        Commands::Deploy => {
            println!("Deploying program");

            let target_deploy_dir = Path::new("target/deploy");
            if !target_deploy_dir.exists() {
                anyhow::bail!("target/deploy directory not found. Please run 'chio build' first.");
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
                anyhow::anyhow!(
                    "No .so file found in target/deploy. Please run 'chio build' first."
                )
            })?;

            let status = Command::new("solana")
                .arg("program")
                .arg("deploy")
                .arg(&so_path)
                .spawn()?
                .wait()
                .with_context(|| "Failed to deploy program")?;

            if !status.success() {
                anyhow::bail!("Deploy failed with exit code: {:?}", status.code());
            } else {
                println!("Program deployed successfully!");
            }
        }
        Commands::Help => {
            display_help_banner()?;
        }
    }

    Ok(())
}

fn display_help_banner() -> Result<()> {
    // banner
    println!(
        r#"
      *     *       
  ___| |__ (_) ___  
 / __| '_ \| |/ _ \ 
| (__| | | | | (_) |
 \___|_| |_|_|\___/ 
 "#
    );

    println!("👾 Setup your pinocchio project blazingly fast💨");

    println!("\n🏗️ AVAILABLE COMMANDS:");
    println!("   chio init <project_name> - Initialize a new Pinocchio project");
    println!("   chio build               - Build the project");
    println!("   chio test                - Run project tests");
    println!("   chio deploy              - Deploy the project");

    Ok(())
}

fn init_project(project_name: &str) -> Result<()> {
    println!(
        r#"
      *     *       
  ___| |__ (_) ___  
 / __| '_ \| |/ _ \ 
| (__| | | | | (_) |
 \___|_| |_|_|\___/ 
                    
 "#
    );
    println!(
        "🧑🏻‍🍳 Initializing your pinocchio project: {}",
        project_name
    );
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"); // Create the project directory
    let project_dir = Path::new(project_name);
    fs::create_dir_all(project_dir)
        .with_context(|| format!("Failed to create project directory: {}", project_name))?;

    // init new cargo project inside
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

    let deploy_dir = project_dir.join("target").join("deploy");
    fs::create_dir_all(&deploy_dir)?;

    // generate keypair
    let keypair_path = format!("./target/deploy/{}-keypair.json", project_name);
    let keygen_output = Command::new("solana-keygen")
        .arg("new")
        .arg("-o")
        .arg(&keypair_path)
        .arg("--no-bip39-passphrase") // skip the passphrase prompt
        .current_dir(project_dir)
        .output()
        .with_context(|| "Failed to generate keypair")?;

    if !keygen_output.status.success() {
        let error = String::from_utf8_lossy(&keygen_output.stderr);
        anyhow::bail!("Failed to generate keypair: {}", error);
    }

    let address_output = Command::new("solana")
        .arg("address")
        .arg("-k")
        .arg(&keypair_path)
        .current_dir(project_dir)
        .output()
        .with_context(|| "Failed to read keypair address")?;

    let program_address: String;
    if address_output.status.success() {
        program_address = String::from_utf8_lossy(&address_output.stdout)
            .trim()
            .to_string();
        println!("Generated program address: {}", program_address);
    } else {
        let error = String::from_utf8_lossy(&address_output.stderr);
        anyhow::bail!("Failed to get program address from keypair: {}", error);
    }

    // get user's wallet address
    let user_address_output = Command::new("solana")
        .arg("address")
        .current_dir(project_dir)
        .output()
        .with_context(|| "Failed to get user address")?;

    let user_address: String;
    if user_address_output.status.success() {
        user_address = String::from_utf8_lossy(&user_address_output.stdout)
            .trim()
            .to_string();
    } else {
        let error = String::from_utf8_lossy(&user_address_output.stderr);
        println!("Failed to get user Solana address: {}", error);
        user_address = String::new();
    }

    create_project_structure(project_dir, user_address, program_address.clone())?;
    update_cargo_toml(project_dir, project_name)?;

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

fn create_project_structure(
    project_dir: &Path,
    user_address: String,
    program_address: String,
) -> Result<()> {
    fs::write(project_dir.join("README.md"), templates::readme_md())?;
    fs::write(project_dir.join(".gitignore"), templates::gitignore())?;

    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    fs::write(
        src_dir.join("lib.rs"),
        templates::lib_rs(program_address.as_str()),
    )?;

    fs::write(src_dir.join("entrypoint.rs"), templates::entrypoint_rs())?;

    fs::write(src_dir.join("errors.rs"), templates::errors_rs())?;

    let instructions_dir = src_dir.join("instructions");
    fs::create_dir_all(&instructions_dir)?;

    fs::write(
        instructions_dir.join("mod.rs"),
        templates::instructions::instructions_mod_rs(),
    )?;
    fs::write(
        instructions_dir.join("initialize.rs"),
        templates::instructions::initilaize(),
    )?;

    let states_dir = src_dir.join("states");
    fs::create_dir_all(&states_dir)?;

    fs::write(
        states_dir.join("mod.rs"),
        templates::states::states_mod_rs(),
    )?;
    fs::write(states_dir.join("utils.rs"), templates::states::utils_rs())?;

    fs::write(states_dir.join("state.rs"), templates::states::state_rs())?;

    let test_dir = project_dir.join("tests");
    fs::create_dir_all(&test_dir)?;

    let test_address = &user_address;

    let project_name = project_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");

    fs::write(
        test_dir.join("tests.rs"),
        templates::unit_tests::unit_test_rs(test_address, &program_address, project_name),
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
pinocchio = "0.8.4"
pinocchio-log = "0.4.0"
pinocchio-pubkey = "0.2.4"
pinocchio-system = "0.2.3"
shank = "0.4.2"

[dev-dependencies]
solana-sdk = "2.2.1"
mollusk-svm = "0.2.0"
mollusk-svm-bencher = "0.2.0" 

[features]
no-entrypoint = []
std = []
test-default = ["no-entrypoint", "std"]
"#,
        project_name
    );

    fs::write(project_dir.join("Cargo.toml"), cargo_toml)?;

    Ok(())
}
