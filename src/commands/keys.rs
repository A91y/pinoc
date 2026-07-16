use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(clap::Subcommand)]
pub enum KeyCommands {
    List,
    Sync,
}

pub fn list_program_keys() -> Result<()> {
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
    println!(
        "{:<20} {:<50} {:<30}",
        "Program", "Public Key", "Keypair File"
    );
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

pub fn sync_program_keys() -> Result<()> {
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

    let src_dir = Path::new("src");
    if !src_dir.exists() {
        anyhow::bail!("src/ directory not found. Please run this command from a project root.");
    }

    match find_program_id_decl(src_dir)? {
        Some(decl) => {
            if decl.address == actual_pubkey {
                println!("✅ Program key is already consistent!");
                println!("🔑 Program ID: {}", actual_pubkey);
                println!("📝 No update needed ({})", decl.file.display());
                return Ok(());
            }
            println!("🔄 Program key mismatch detected:");
            println!("   Current in {}: {}", decl.file.display(), decl.address);
            println!("   Actual keypair:   {}", actual_pubkey);

            let content = fs::read_to_string(&decl.file)
                .with_context(|| format!("Failed to read {}", decl.file.display()))?;
            // Replace only this address literal, preserving the declaration form.
            let updated = content.replacen(
                &format!("\"{}\"", decl.address),
                &format!("\"{}\"", actual_pubkey),
                1,
            );
            fs::write(&decl.file, updated)
                .with_context(|| format!("Failed to write {}", decl.file.display()))?;

            println!("✅ Successfully synced program key!");
            println!("🔑 Program ID: {}", actual_pubkey);
            println!("📝 Updated {}", decl.file.display());
        }
        None => {
            println!("⚠️  No program ID declaration found under src/.");
            println!("💡 Declare your program ID with one of:");
            println!("   pinocchio::address::declare_id!(\"{}\");", actual_pubkey);
            println!(
                "   pub const ID: Address = Address::from_str_const(\"{}\");",
                actual_pubkey
            );
        }
    }

    Ok(())
}

fn extract_project_name(cargo_content: &str) -> Option<String> {
    // Parse the manifest rather than line-matching: `name = "..."` may be
    // aligned with padding spaces (`name    = "..."`) or otherwise formatted in
    // any way valid TOML allows.
    let manifest: toml::Value = toml::from_str(cargo_content).ok()?;
    manifest
        .get("package")?
        .get("name")?
        .as_str()
        .map(str::to_string)
}

struct ProgramIdDecl {
    file: PathBuf,
    address: String,
}

/// Finds the program's own ID declaration anywhere under `src_dir`, in priority
/// order: any `declare_id!("...")` macro (regardless of path prefix), else a
/// `const ID` initialized from a string literal (`Address::from_str_const("...")`,
/// `pubkey!("...")`, or a bare `"..."`). Returns the file the declaration lives
/// in and the current address, since the ID isn't always a `declare_id!` in
/// `lib.rs` (e.g. pinocchio 0.11 programs use `const ID` in `constants.rs`).
fn find_program_id_decl(src_dir: &Path) -> Result<Option<ProgramIdDecl>> {
    let mut files = Vec::new();
    collect_rust_files(src_dir, &mut files)?;
    files.sort();

    let mut const_fallback: Option<ProgramIdDecl> = None;
    for path in files {
        let Ok(src) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(file) = syn::parse_file(&src) else {
            continue;
        };
        if let Some(address) = find_declare_id(&file.items) {
            return Ok(Some(ProgramIdDecl {
                file: path,
                address,
            }));
        }
        if const_fallback.is_none() {
            if let Some(address) = find_const_id(&file.items) {
                const_fallback = Some(ProgramIdDecl {
                    file: path,
                    address,
                });
            }
        }
    }
    Ok(const_fallback)
}

fn collect_rust_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_rust_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

/// Any `declare_id!("...")` invocation, regardless of macro path prefix,
/// including inside inline modules.
fn find_declare_id(items: &[syn::Item]) -> Option<String> {
    for item in items {
        match item {
            syn::Item::Macro(m) if macro_name_is(&m.mac, "declare_id") => {
                if let Some(addr) = lit_str_from_macro(&m.mac) {
                    return Some(addr);
                }
            }
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    if let Some(addr) = find_declare_id(inner) {
                        return Some(addr);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// A `const ID` whose initializer carries a string literal
/// (`Address::from_str_const("...")`, `pubkey!("...")`, or a bare `"..."`),
/// including inside inline modules.
fn find_const_id(items: &[syn::Item]) -> Option<String> {
    for item in items {
        match item {
            syn::Item::Const(c) if c.ident == "ID" => {
                if let Some(addr) = lit_str_from_expr(&c.expr) {
                    return Some(addr);
                }
            }
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    if let Some(addr) = find_const_id(inner) {
                        return Some(addr);
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn macro_name_is(mac: &syn::Macro, name: &str) -> bool {
    mac.path
        .segments
        .last()
        .map(|s| s.ident == name)
        .unwrap_or(false)
}

fn lit_str_from_macro(mac: &syn::Macro) -> Option<String> {
    mac.parse_body::<syn::LitStr>().ok().map(|l| l.value())
}

/// Pulls a string literal out of `f("...")`, `m!("...")`, or a bare `"..."`.
fn lit_str_from_expr(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(s),
            ..
        }) => Some(s.value()),
        syn::Expr::Call(call) => call.args.iter().find_map(lit_str_from_expr),
        syn::Expr::MethodCall(mc) => mc.args.iter().find_map(lit_str_from_expr),
        syn::Expr::Macro(m) => lit_str_from_macro(&m.mac),
        _ => None,
    }
}
