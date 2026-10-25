use crate::client_gen::{self, Language};
use crate::config;
use crate::idl::{codama_native, Generator};
use anyhow::{Context, Result};
use std::io::{IsTerminal, Write};
use std::path::Path;

#[derive(clap::Subcommand)]
pub enum ClientCommands {
    Generate {
        #[arg(
            long,
            help = "Output directory for the generated client [default: clients/rust-shank or clients/rust-codama, depending on the resolved generator, or clients/ts for --language ts, so all can coexist]"
        )]
        out_dir: Option<String>,
        #[arg(
            long,
            help = "Path to the IDL JSON [default: [idl].out_dir in Pinoc.toml, else target/idl]"
        )]
        idl_dir: Option<String>,
        #[arg(
            long,
            value_enum,
            help = "Which generator to use: shank (built-in, no setup) or codama (Node.js, richer output). Prompts interactively if omitted."
        )]
        generator: Option<Generator>,
        #[arg(
            long,
            value_enum,
            help = "Client language. ts (TypeScript) is rendered by the codama generator only [default: [client].languages in Pinoc.toml, else rust]"
        )]
        language: Option<Language>,
        #[arg(
            long,
            help = "Automatically run 'npm install' for the codama generator if its dependencies aren't present yet"
        )]
        auto_install: bool,
        #[arg(
            short = 'y',
            long = "yes",
            help = "Skip the confirmation prompt when --generator contradicts detected Codama macros"
        )]
        yes: bool,
        #[arg(
            long,
            help = "Force CPI variant generation (XxxCpi/XxxCpiBuilder) in the shank generator, skipping auto-detection",
            conflicts_with = "no_cpi"
        )]
        with_cpi: bool,
        #[arg(
            long,
            help = "Never generate CPI variants in the shank generator, regardless of auto-detection",
            conflicts_with = "with_cpi"
        )]
        no_cpi: bool,
        #[arg(
            long,
            help = "Generate, report any file that differs from what is in the output directory, and leave the directory as it was; exits nonzero if one does"
        )]
        check: bool,
    },
}

pub struct GenerateArgs<'a> {
    pub idl_dir: Option<&'a str>,
    pub out_dir: Option<&'a str>,
    pub generator: Option<Generator>,
    pub language: Option<Language>,
    pub check: bool,
    pub auto_install: bool,
    pub yes: bool,
    pub with_cpi: bool,
    pub no_cpi: bool,
}

/// Generates the client for `--language`, else for each of `[client].languages`.
pub fn generate_client(args: GenerateArgs) -> Result<()> {
    let client_config = config::load()?.map(|c| c.client).unwrap_or_default();
    let languages = match args.language {
        Some(language) => vec![language],
        None if !client_config.languages.is_empty() => {
            let mut languages = client_config.languages.clone();
            languages.dedup();
            languages
        }
        None => vec![Language::Rust],
    };
    if languages.len() > 1 && args.out_dir.is_some() {
        anyhow::bail!(
            "--out-dir names one directory, but [client].languages in Pinoc.toml asks for {} clients. Pass --language with it.",
            languages.len()
        );
    }
    let idl_dir = match args.idl_dir {
        Some(dir) => dir.to_string(),
        None => config::idl_out_dir()?,
    };
    for language in languages {
        // `[client].generator` chooses between the Rust renderers; TypeScript has one.
        let generator = match language {
            Language::Rust => args.generator.or(client_config.generator),
            Language::Ts => args.generator,
        };
        let one = OneClient {
            idl_dir: &idl_dir,
            out_dir: args.out_dir,
            generator,
            language,
            auto_install: args.auto_install,
            yes: args.yes,
            with_cpi: args.with_cpi,
            no_cpi: args.no_cpi,
        };
        if args.check {
            check_client(one)?;
        } else {
            generate_one(one)?;
        }
    }
    Ok(())
}

struct OneClient<'a> {
    idl_dir: &'a str,
    out_dir: Option<&'a str>,
    generator: Option<Generator>,
    language: Language,
    auto_install: bool,
    yes: bool,
    with_cpi: bool,
    no_cpi: bool,
}

/// Generates in place, compares, then puts every file back as it was. The
/// codama tooling under `.pinoc-codama/` is left installed.
fn check_client(one: OneClient) -> Result<()> {
    let generator = resolve_generator(one.generator, one.language, one.yes)?;
    let out_dir = resolve_out_dir(one.out_dir, generator, one.language)?;
    let root = Path::new(&out_dir);
    let before = snapshot(root)?;
    let language = one.language;
    let generated = generate_one(OneClient {
        out_dir: Some(&out_dir),
        generator: Some(generator),
        yes: true,
        ..one
    });
    let after = snapshot(root);
    restore(root, &before)?;
    generated?;
    let after = after?;

    let mut stale = Vec::new();
    for (rel, bytes) in &after {
        match before.get(rel) {
            Some(old) if old == bytes => {}
            Some(_) => stale.push(format!("{} (differs)", rel.display())),
            None => stale.push(format!("{} (missing)", rel.display())),
        }
    }
    for rel in before.keys().filter(|rel| !after.contains_key(*rel)) {
        if is_generated(rel) {
            stale.push(format!("{} (no longer generated)", rel.display()));
        }
    }
    stale.sort();
    super::idl::report_stale(
        &stale,
        &format!("The {} client in {out_dir}/", language.label()),
        "Run `pinoc client generate` and commit the result.",
    )
}

/// Files under `src/generated/` are the renderer's own, so one it no longer
/// writes is stale. Anything else in the client directory may be hand-written.
fn is_generated(rel: &Path) -> bool {
    rel.starts_with("src/generated")
}

type Snapshot = std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>;

fn snapshot(root: &Path) -> Result<Snapshot> {
    fn walk(dir: &Path, root: &Path, out: &mut Snapshot) -> Result<()> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return Ok(());
        };
        for entry in entries {
            let path = entry?.path();
            if path.file_name().and_then(|n| n.to_str()) == Some(".pinoc-codama") {
                continue;
            }
            if path.is_dir() {
                walk(&path, root, out)?;
            } else {
                out.insert(
                    path.strip_prefix(root)?.to_path_buf(),
                    std::fs::read(&path)?,
                );
            }
        }
        Ok(())
    }
    let mut out = Snapshot::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

fn restore(root: &Path, before: &Snapshot) -> Result<()> {
    for rel in snapshot(root)?
        .keys()
        .filter(|rel| !before.contains_key(*rel))
    {
        std::fs::remove_file(root.join(rel))?;
    }
    for (rel, bytes) in before {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if std::fs::read(&path).ok().as_deref() != Some(bytes) {
            std::fs::write(&path, bytes)?;
        }
    }
    Ok(())
}

struct Project {
    lib_name: String,
    src_dir: std::path::PathBuf,
    codama_usage: codama_native::CodamaUsage,
}

fn project() -> Result<Project> {
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
    let codama_usage = codama_native::codama_usage(&crate_root, &src_dir)?;
    Ok(Project {
        lib_name,
        src_dir,
        codama_usage,
    })
}

fn resolve_generator(
    generator: Option<Generator>,
    language: Language,
    yes: bool,
) -> Result<Generator> {
    let codama_usage = project()?.codama_usage;
    let detected_codama = codama_usage.detected();

    if language == Language::Ts && generator == Some(Generator::Shank) {
        anyhow::bail!(
            "--language ts needs --generator codama: the built-in shank generator only renders Rust."
        );
    }

    Ok(match generator {
        // TypeScript has a single renderer, so there is no choice to confirm or prompt for.
        _ if language == Language::Ts => Generator::Codama,
        Some(g) => {
            let contradicts = matches!(
                (g, detected_codama),
                (Generator::Codama, false) | (Generator::Shank, true)
            );
            if contradicts && !yes {
                confirm_contradicting_choice(g, &codama_usage)?;
            }
            g
        }
        None => prompt_for_generator(detected_codama),
    })
}

fn generate_one(one: OneClient) -> Result<()> {
    let OneClient {
        idl_dir,
        out_dir,
        generator,
        language,
        auto_install,
        yes,
        with_cpi,
        no_cpi,
    } = one;
    let Project {
        lib_name, src_dir, ..
    } = project()?;
    let generator = resolve_generator(generator, language, yes)?;

    let out_dir = resolve_out_dir(out_dir, generator, language)?;
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
                let reason = if with_cpi {
                    "forced via --with-cpi"
                } else {
                    "detected invoke()/invoke_signed() usage in program source"
                };
                println!("🔌 CPI variants: generating ({reason})");
            }

            let enums = client_gen::discriminants::for_idl(&idl_path, &src_dir)?;
            if !enums.is_empty() {
                println!("{}", client_gen::discriminants::describe(&enums));
            }
            client_gen::shank::generate_rust_client(
                &idl_path,
                Path::new(out_dir),
                generate_cpi,
                &enums,
            )
            .with_context(|| "Failed to generate Rust client")?;
            println!("✅ Rust client written to {out_dir}/");
        }
        Generator::Codama => {
            let label = language.label();
            println!("🧬 Generating {label} client (codama, Node.js)...");
            let idl_path = Path::new(idl_dir).join(format!("{lib_name}.codama.json"));
            if !idl_path.exists() {
                anyhow::bail!(
                    "Codama-compatible IDL not found at {}. Run 'pinoc build' or 'pinoc idl' first.",
                    idl_path.display()
                );
            }
            // Resolved before rendering, so an enum that cannot be generated
            // correctly stops the run without leaving a client behind.
            let enums = client_gen::discriminants::for_idl(&idl_path, &src_dir)
                .with_context(|| format!("Failed to generate {label} client via codama"))?;
            if !enums.is_empty() {
                println!("{}", client_gen::discriminants::describe(&enums));
            }
            client_gen::codama::generate_via_codama(
                &idl_path,
                Path::new(out_dir),
                auto_install,
                language,
                &enums,
            )
            .with_context(|| format!("Failed to generate {label} client via codama"))?;
            println!("✅ {label} client written to {out_dir}/ (via codama)");
        }
    }

    Ok(())
}

/// Resolves the output directory: explicit `--out-dir` > `Pinoc.toml`'s
/// per-generator `shank_out_dir`/`codama_out_dir` > `Pinoc.toml`'s shared
/// `out_dir` (warns every time, since both generators would write there) >
/// the dynamic per-generator default (`clients/rust-shank`/`clients/rust-codama`).
/// Those paths configure the Rust clients; TypeScript goes to `[client].ts_out_dir`,
/// else `clients/ts`.
fn resolve_out_dir(
    out_dir: Option<&str>,
    generator: Generator,
    language: Language,
) -> Result<String> {
    if let Some(explicit) = out_dir {
        return Ok(explicit.to_string());
    }
    let client_config = config::read_pinoc_config_optional()?
        .map(|c| c.client)
        .unwrap_or_default();
    if language == Language::Ts {
        return Ok(client_config
            .ts_out_dir
            .unwrap_or_else(|| "clients/ts".to_string()));
    }

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
fn confirm_contradicting_choice(
    chosen: Generator,
    codama_usage: &codama_native::CodamaUsage,
) -> Result<()> {
    let (chosen_name, reason) = match chosen {
        Generator::Codama if codama_usage.derives => (
            "codama",
            "Codama derives were found, but neither `codama` nor `codama-macros` is a dependency of this program",
        ),
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
    let recommended = if detected_codama {
        Generator::Codama
    } else {
        Generator::Shank
    };

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
        if detected_codama {
            ""
        } else {
            " (recommended)"
        }
    );
    println!(
        "  2) codama - Node.js-based, richer output (CPI helpers, RPC fetch helpers){}",
        if detected_codama {
            " (recommended)"
        } else {
            ""
        }
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
