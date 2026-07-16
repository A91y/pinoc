use super::banner::BANNER;
use anyhow::Result;

pub fn display_help_banner() -> Result<()> {
    println!("{BANNER}");

    println!("👾 Setup your pinocchio project blazingly fast💨");

    println!("\n🏗️ AVAILABLE COMMANDS:");
    println!("   pinoc init <project_name> [--no-git] [--with-example] - Initialize a new Pinocchio project");
    println!(
        "   pinoc build               - Build the project (also regenerates target/idl/*.json)"
    );
    println!("   pinoc test                - Run project tests");
    println!("   pinoc deploy [--cluster] [--wallet] - Deploy the project (uses Pinoc.toml config, optional overrides)");
    println!(
        "   pinoc clean [--no-preserve] - Clean target directory (preserves keypairs by default)"
    );
    println!("   pinoc add <package_name>  - Add a package to the project");
    println!("   pinoc search [query]      - Search for pinocchio packages on crates.io");
    println!("   pinoc keys list           - List program keypairs");
    println!("   pinoc keys sync           - Sync program ID with keypair");
    println!("   pinoc idl [--out-dir]     - Regenerate the IDL JSON (also runs automatically on 'pinoc build')");
    println!(
        "   pinoc client generate [--generator shank|codama] - Generate a Rust client from the IDL"
    );
    println!("   pinoc config init [-y]    - Create a Pinoc.toml for the current project");

    Ok(())
}
