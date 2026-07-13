use crate::client_gen;
use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum Generator {
    Shank,
    Codama,
}

#[derive(clap::Subcommand)]
pub enum ClientCommands {
    Generate {
        #[arg(long, help = "Output directory for the generated Rust client", default_value = "clients/rust")]
        out_dir: String,
        #[arg(long, help = "Path to the IDL JSON", default_value = "target/idl")]
        idl_dir: String,
        #[arg(long, value_enum, help = "Which generator to use: shank (built-in, no setup) or codama (Node.js, richer output). Prompts interactively if omitted.")]
        generator: Option<Generator>,
        #[arg(long, help = "Automatically run 'npm install' for the codama generator if its dependencies aren't present yet")]
        auto_install: bool,
    },
}

pub fn generate_client(
    idl_dir: &str,
    out_dir: &str,
    generator: Option<Generator>,
    auto_install: bool,
) -> Result<()> {
    let cargo_toml = Path::new("Cargo.toml");
    if !cargo_toml.exists() {
        anyhow::bail!("Cargo.toml not found. Please run this command from the project root.");
    }
    let manifest = shank_idl::manifest::Manifest::from_path(cargo_toml)
        .with_context(|| "Failed to read Cargo.toml")?;
    let lib_name = manifest.lib_name()?;

    let generator = generator.unwrap_or_else(prompt_for_generator);

    match generator {
        Generator::Shank => {
            println!("🧬 Generating Rust client (shank, built-in)...");
            let idl_path = Path::new(idl_dir).join(format!("{lib_name}.json"));
            if !idl_path.exists() {
                anyhow::bail!(
                    "IDL not found at {}. Run 'pinoc build' or 'pinoc idl' first.",
                    idl_path.display()
                );
            }
            client_gen::shank::generate_rust_client(&idl_path, Path::new(out_dir))
                .with_context(|| "Failed to generate Rust client")?;
            println!("✅ Rust client written to {out_dir}/");
        }
        Generator::Codama => {
            println!("🧬 Generating Rust client (codama, Node.js)...");
            let idl_path = Path::new(idl_dir).join(format!("{lib_name}.codama.json"));
            if !idl_path.exists() {
                anyhow::bail!(
                    "Codama-compatible IDL not found at {}. Run 'pinoc build' or 'pinoc idl' first.",
                    idl_path.display()
                );
            }
            client_gen::codama::generate_via_codama(&idl_path, Path::new(out_dir), auto_install)
                .with_context(|| "Failed to generate Rust client via codama")?;
            println!("✅ Rust client written to {out_dir}/ (via codama)");
        }
    }

    Ok(())
}

fn prompt_for_generator() -> Generator {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return Generator::Shank;
    }

    println!("Which client generator would you like to use?");
    println!("  1) shank  - built into pinoc, no setup required (default)");
    println!("  2) codama - Node.js-based, richer output (CPI helpers, RPC fetch helpers)");
    print!("Choice [1]: ");
    let _ = std::io::stdout().flush();

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        return Generator::Shank;
    }
    match input.trim() {
        "2" | "codama" => Generator::Codama,
        _ => Generator::Shank,
    }
}
