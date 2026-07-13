use super::banner::BANNER;
use crate::templates;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn init_project(project_name: &str, no_git: bool, with_example: bool) -> Result<()> {
    if !is_valid_project_name(project_name) {
        anyhow::bail!(
            "Invalid project name '{}'. Project names can only contain letters, numbers, and underscores (_). \
            Hyphens (-) and other special characters are not allowed.",
            project_name
        );
    }

    println!("{BANNER}");
    println!("🧑🏻‍🍳 Initializing your pinocchio project: {}", project_name);
    println!();
    let project_dir = Path::new(project_name);
    fs::create_dir_all(project_dir)
        .with_context(|| format!("Failed to create project directory: {}", project_name))?;

    let mut cargo_init = Command::new("cargo");
    cargo_init
        .arg("init")
        .arg("--lib")
        .arg("--name")
        .arg(project_name);

    if no_git {
        cargo_init.arg("--vcs").arg("none");
    }

    let output = cargo_init
        .current_dir(project_dir)
        .output()
        .with_context(|| "Failed to run 'cargo init'")?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Failed to initialize Cargo project: {}", error);
    }

    let deploy_dir = project_dir.join("target").join("deploy");
    fs::create_dir_all(&deploy_dir)?;

    let keypair_path = format!("./target/deploy/{}-keypair.json", project_name);
    let keygen_output = Command::new("solana-keygen")
        .arg("new")
        .arg("-o")
        .arg(&keypair_path)
        .arg("--no-bip39-passphrase")
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

    let user_address_output = Command::new("solana")
        .arg("address")
        .current_dir(project_dir)
        .output()
        .with_context(|| "Failed to get user address")?;

    let user_address = if user_address_output.status.success() {
        String::from_utf8_lossy(&user_address_output.stdout)
            .trim()
            .to_string()
    } else {
        let error = String::from_utf8_lossy(&user_address_output.stderr);
        println!("Failed to get user Solana address: {}", error);
        String::new()
    };

    if with_example {
        create_project_structure(project_dir, user_address, program_address.clone())?;
        update_cargo_toml(project_dir, project_name)?;
    } else {
        create_minimal_project_structure(project_dir, project_name, program_address.clone())?;
    }

    if !no_git {
        init_git_repo(project_dir, project_name)?;
    }

    println!();
    println!(
        "✅ Pinocchio Project '{}' initialized successfully!",
        project_name
    );
    println!("\n📋 Next steps:");
    println!("$ cd {}", project_name);
    println!("$ pinoc build");
    println!("$ pinoc test");
    println!("$ pinoc deploy");
    println!();

    Ok(())
}

fn create_minimal_project_structure(
    project_dir: &Path,
    project_name: &str,
    program_address: String,
) -> Result<()> {
    println!("📦 Creating minimal project structure...");

    fs::write(
        project_dir.join("Cargo.toml"),
        templates::minimal::cargo_toml(project_name),
    )?;

    fs::write(project_dir.join(".gitignore"), templates::gitignore())?;

    let src_dir = project_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    fs::write(
        src_dir.join("lib.rs"),
        templates::minimal::lib_rs(&program_address),
    )?;

    fs::write(
        project_dir.join("README.md"),
        templates::minimal::readme_md(project_name),
    )?;

    fs::write(project_dir.join("Pinoc.toml"), templates::pinoc_toml())?;

    println!("✅ Minimal project structure created!");
    println!("📁 Only essential files generated: Cargo.toml, src/lib.rs, README.md, .gitignore, Pinoc.toml");

    Ok(())
}

fn is_valid_project_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }

    name.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn init_git_repo(project_dir: &Path, project_name: &str) -> Result<()> {
    let git_init_output = Command::new("git")
        .arg("init")
        .current_dir(project_dir)
        .output()
        .with_context(|| "Failed to initialize git repository")?;

    if !git_init_output.status.success() {
        let error = String::from_utf8_lossy(&git_init_output.stderr);
        println!("Warning: Failed to initialize git repository: {}", error);
        return Ok(());
    }

    let git_add_output = Command::new("git")
        .arg("add")
        .arg(".")
        .current_dir(project_dir)
        .output()
        .with_context(|| "Failed to add files to git")?;

    if !git_add_output.status.success() {
        let error = String::from_utf8_lossy(&git_add_output.stderr);
        println!("Warning: Failed to add files to git: {}", error);
        return Ok(());
    }

    let commit_message = format!("Initial commit: Setup Pinocchio project '{}'", project_name);
    let git_commit_output = Command::new("git")
        .arg("commit")
        .arg("-m")
        .arg(&commit_message)
        .current_dir(project_dir)
        .output()
        .with_context(|| "Failed to make initial commit")?;

    if !git_commit_output.status.success() {
        let error = String::from_utf8_lossy(&git_commit_output.stderr);
        println!("Warning: Failed to make initial commit: {}", error);
        if error.contains("user.email") || error.contains("user.name") {
            println!("Hint: Set your git config with:");
            println!("  git config --global user.email \"you@example.com\"");
            println!("  git config --global user.name \"Your Name\"");
        }
        return Ok(());
    }
    Ok(())
}

fn create_project_structure(
    project_dir: &Path,
    user_address: String,
    program_address: String,
) -> Result<()> {
    fs::write(project_dir.join("README.md"), templates::readme_md())?;
    fs::write(project_dir.join(".gitignore"), templates::gitignore())?;
    fs::write(project_dir.join("Pinoc.toml"), templates::pinoc_toml())?;

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
        templates::instructions::initialize(),
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
    fs::write(
        project_dir.join("Cargo.toml"),
        templates::cargo_toml(project_name),
    )?;

    Ok(())
}
