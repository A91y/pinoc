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
enum KeyCommands {
    List,
    Sync,
}

#[derive(Subcommand)]
enum Commands {
    Init {
        project_name: String,
        #[arg(long, help = "Don't initialize git")]
        no_git: bool,
    },
    Build,
    Test,
    Deploy,
    Clean {
        #[arg(long, help = "Remove all files including keypair files")]
        no_preserve: bool,
    },
    Add {
        package_name: String,
    },
    Search {
        query: Option<String>,
    },
    Keys {
        #[command(subcommand)]
        command: KeyCommands,
    },
    #[command(name = "--help")]
    Help,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init {
            project_name,
            no_git,
        } => {
            init_project(project_name, *no_git)?;
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
                anyhow::anyhow!(
                    "No .so file found in target/deploy. Please run 'pinoc build' first."
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
        Commands::Clean { no_preserve } => {
            clean_project(*no_preserve)?;
        }
        Commands::Add { package_name } => {
            add_package(package_name)?;
        }
        Commands::Search { query } => {
            search_packages(query.as_deref())?;
        }
        Commands::Keys { command } => match command {
            KeyCommands::List => {
                list_program_keys()?;
            }
            KeyCommands::Sync => {
                sync_program_keys()?;
            }
        },
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
       _                   
 _ __ (_)_ __   ___   ___  
| '_ \| | '_ \ / _ \ / __| 
| |_) | | | | | (_) | (__  
| .__/|_|_| |_|\___/ \___| 
|_|                       
 "#
    );

    println!("👾 Setup your pinocchio project blazingly fast💨");

    println!("\n🏗️ AVAILABLE COMMANDS:");
    println!("   pinoc init <project_name> - Initialize a new Pinocchio project");
    println!("   pinoc build               - Build the project");
    println!("   pinoc test                - Run project tests");
    println!("   pinoc deploy              - Deploy the project");
    println!(
        "   pinoc clean [--no-preserve] - Clean target directory (preserves keypairs by default)"
    );
    println!("   pinoc add <package_name>  - Add a package to the project");
    println!("   pinoc search [query]      - Search for pinocchio packages on crates.io");

    Ok(())
}

fn init_project(project_name: &str, no_git: bool) -> Result<()> {
    // Validate project name - only allow alphanumeric characters and underscores
    if !is_valid_project_name(project_name) {
        anyhow::bail!(
            "Invalid project name '{}'. Project names can only contain letters, numbers, and underscores (_). \
            Hyphens (-) and other special characters are not allowed.",
            project_name
        );
    }

    println!(
        r#"
       _                   
 _ __ (_)_ __   ___   ___  
| '_ \| | '_ \ / _ \ / __| 
| |_) | | | | | (_) | (__  
| .__/|_|_| |_|\___/ \___| 
|_|                       
 "#
    );
    println!("🧑🏻‍🍳 Initializing your pinocchio project: {}", project_name);
    println!(""); // Create the project directory
    let project_dir = Path::new(project_name);
    fs::create_dir_all(project_dir)
        .with_context(|| format!("Failed to create project directory: {}", project_name))?;

    // init new cargo project inside
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

    if !no_git {
        init_git_repo(project_dir, project_name)?;
    }

    println!("");
    println!(
        "✅ Pinocchio Project '{}' initialized successfully!",
        project_name
    );
    println!("\n📋 Next steps:");
    println!("$ cd {}", project_name);
    println!("$ pinoc build");
    println!("$ pinoc test");
    println!("$ pinoc deploy");
    println!("");

    Ok(())
}

/// validates that the project name only contains alphanumeric characters and underscores
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
        // Check if it's because of missing git config
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
solana-sdk = "2.3.0"
solana-program-runtime = "=2.3.1"
mollusk-svm = "0.3.0"
mollusk-svm-bencher = "0.3.0" 

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

fn add_package(package_name: &str) -> Result<()> {
    // Check if Cargo.toml exists
    let cargo_toml_path = Path::new("Cargo.toml");
    if !cargo_toml_path.exists() {
        anyhow::bail!(
            "Cargo.toml not found. Please run this command from the project root directory."
        );
    }

    // Add the package using cargo add
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

fn search_packages(query: Option<&str>) -> Result<()> {
    let search_term = match query {
        Some(q) => format!("pinocchio {}", q),
        None => "pinocchio".to_string(),
    };

    println!("🔍 Searching for packages matching '{}'...\n", search_term);

    // Run cargo search
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

#[derive(Debug)]
struct SearchResult {
    name: String,
    description: String,
    version: String,
}

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

fn clean_project(no_preserve: bool) -> Result<()> {
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

    fs::remove_dir_all(target_dir).with_context(|| "Failed to remove target directory")?;

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

fn list_program_keys() -> Result<()> {
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

                    // Get the public key from the keypair
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
    println!("{:<20} {:<50} {}", "Program", "Public Key", "Keypair File");
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

fn sync_program_keys() -> Result<()> {
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

    // Get the actual program public key
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

    // Find and update the declare_id! macro in lib.rs
    let lib_rs_path = Path::new("src/lib.rs");
    if !lib_rs_path.exists() {
        anyhow::bail!("src/lib.rs not found");
    }

    let lib_content =
        fs::read_to_string(lib_rs_path).with_context(|| "Failed to read src/lib.rs")?;

    // Check if the key is already consistent
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
            // Look for the pattern: declare_id!("...") or pinocchio_pubkey::declare_id!("...")
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
