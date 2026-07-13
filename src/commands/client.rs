use crate::client_gen;
use crate::config;
use crate::idl::{codama_native, Generator};
use anyhow::{Context, Result};
use std::io::{IsTerminal, Write};
use std::path::Path;

#[derive(clap::Subcommand)]
pub enum ClientCommands {
    Generate {
        #[arg(long, help = "Output directory for the generated Rust client [default: clients/rust-shank or clients/rust-codama, depending on the resolved generator, so both can coexist]")]
        out_dir: Option<String>,
        #[arg(long, help = "Path to the IDL JSON", default_value = "target/idl")]
        idl_dir: String,
        #[arg(long, value_enum, help = "Which generator to use: shank (built-in, no setup) or codama (Node.js, richer output). Prompts interactively if omitted.")]
        generator: Option<Generator>,
        #[arg(long, help = "Automatically run 'npm install' for the codama generator if its dependencies aren't present yet")]
        auto_install: bool,
        #[arg(short = 'y', long = "yes", help = "Skip the confirmation prompt when --generator contradicts detected Codama macros")]
        yes: bool,
        #[arg(long, help = "Force CPI variant generation (XxxCpi/XxxCpiBuilder) in the shank generator, skipping auto-detection", conflicts_with = "no_cpi")]
        with_cpi: bool,
        #[arg(long, help = "Never generate CPI variants in the shank generator, regardless of auto-detection", conflicts_with = "with_cpi")]
        no_cpi: bool,
    },
}

pub fn generate_client(
    idl_dir: &str,
    out_dir: Option<&str>,
    generator: Option<Generator>,
    auto_install: bool,
    yes: bool,
    with_cpi: bool,
    no_cpi: bool,
) -> Result<()> {
    let crate_root = std::env::current_dir().with_context(|| "Failed to read current directory")?;
    let cargo_toml = crate_root.join("Cargo.toml");
    if !cargo_toml.exists() {
        anyhow::bail!("Cargo.toml not found. Please run this command from the project root.");
    }
    let manifest = shank_idl::manifest::Manifest::from_path(&cargo_toml)
        .with_context(|| "Failed to read Cargo.toml")?;
    let lib_name = manifest.lib_name()?;
    let src_dir = manifest
        .lib_rel_path()
        .map(|p| crate_root.join(p))
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| crate_root.clone());

    let detected_codama = codama_native::codama_macros_detected(&crate_root, &src_dir)?;

    let generator = match generator {
        Some(g) => {
            let contradicts = matches!(
                (g, detected_codama),
                (Generator::Codama, false) | (Generator::Shank, true)
            );
            if contradicts && !yes {
                confirm_contradicting_choice(g)?;
            }
            g
        }
        None => prompt_for_generator(detected_codama),
    };

    let out_dir = resolve_out_dir(out_dir, generator)?;
    let out_dir = out_dir.as_str();

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

            let generate_cpi = if no_cpi {
                false
            } else if with_cpi {
                true
            } else {
                client_gen::shank::cpi::cpi_usage_detected(&src_dir)?
            };
            if generate_cpi {
                let reason = if with_cpi { "forced via --with-cpi" } else { "detected invoke()/invoke_signed() usage in program source" };
                println!("🔌 CPI variants: generating ({reason})");
            }

            client_gen::shank::generate_rust_client(&idl_path, Path::new(out_dir), generate_cpi)
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

/// Resolves the output directory: explicit `--out-dir` > `Pinoc.toml`'s
/// per-generator `shank_out_dir`/`codama_out_dir` > `Pinoc.toml`'s shared
/// `out_dir` (warns every time, since both generators would write there) >
/// the dynamic per-generator default (`clients/rust-shank`/`clients/rust-codama`).
fn resolve_out_dir(out_dir: Option<&str>, generator: Generator) -> Result<String> {
    if let Some(explicit) = out_dir {
        return Ok(explicit.to_string());
    }

    let client_config = config::read_pinoc_config_optional()?
        .map(|c| c.client)
        .unwrap_or_default();
    let per_generator = match generator {
        Generator::Shank => client_config.shank_out_dir,
        Generator::Codama => client_config.codama_out_dir,
    };
    if let Some(dir) = per_generator {
        return Ok(dir);
    }
    if let Some(shared) = client_config.out_dir {
        println!(
            "⚠️  Using [client].out_dir {shared:?} from Pinoc.toml, shared by both generators. Generating with the other one will overwrite this output."
        );
        return Ok(shared);
    }

    Ok(match generator {
        Generator::Shank => "clients/rust-shank".to_string(),
        Generator::Codama => "clients/rust-codama".to_string(),
    })
}

/// Bails with `-y` guidance when non-interactive; otherwise asks for explicit
/// confirmation before generating with a choice that contradicts detection.
fn confirm_contradicting_choice(chosen: Generator) -> Result<()> {
    let (chosen_name, reason) = match chosen {
        Generator::Codama => ("codama", "no Codama macros were detected in this program"),
        Generator::Shank => ("shank", "Codama macros were detected in this program"),
    };
    if !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "Refusing to generate with '{chosen_name}' without confirmation ({reason}). Pass -y/--yes to proceed non-interactively."
        );
    }
    print!("You chose '{chosen_name}', but {reason}. Continue anyway? [y/N]: ");
    let _ = std::io::stdout().flush();
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
        anyhow::bail!("Aborted.");
    }
    Ok(())
}

fn prompt_for_generator(detected_codama: bool) -> Generator {
    let recommended = if detected_codama { Generator::Codama } else { Generator::Shank };

    if !std::io::stdin().is_terminal() {
        return recommended;
    }

    if detected_codama {
        println!("ℹ️  Codama macros detected in this program; recommending codama.");
    } else {
        println!("ℹ️  No Codama macros detected in this program; recommending shank.");
    }
    println!("Which client generator would you like to use?");
    println!(
        "  1) shank  - built into pinoc, no setup required{}",
        if detected_codama { "" } else { " (recommended)" }
    );
    println!(
        "  2) codama - Node.js-based, richer output (CPI helpers, RPC fetch helpers){}",
        if detected_codama { " (recommended)" } else { "" }
    );
    print!("Choice [{}]: ", if detected_codama { "2" } else { "1" });
    let _ = std::io::stdout().flush();

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_err() {
        return recommended;
    }
    match input.trim() {
        "1" | "shank" => Generator::Shank,
        "2" | "codama" => Generator::Codama,
        _ => recommended,
    }
}
