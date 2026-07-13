pub mod codama;
pub mod manual_errors;

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn generate_idl(out_dir: &str, program_id: Option<&str>) -> Result<()> {
    println!("🧩 Generating IDL...");

    let crate_root = std::env::current_dir().with_context(|| "Failed to read current directory")?;
    let cargo_toml = crate_root.join("Cargo.toml");
    if !cargo_toml.exists() {
        anyhow::bail!("Cargo.toml not found. Please run this command from the project root.");
    }

    let manifest = shank_idl::manifest::Manifest::from_path(&cargo_toml)
        .with_context(|| "Failed to read Cargo.toml")?;
    let lib_rel_path = manifest
        .lib_rel_path()
        .ok_or_else(|| anyhow::anyhow!("Cargo.toml does not declare a [lib] target"))?;
    let lib_path = crate_root.join(lib_rel_path);
    let lib_path_str = lib_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid path: {}", lib_path.display()))?;

    let opts = shank_idl::ParseIdlOpts {
        program_address_override: program_id.map(String::from),
        ..shank_idl::ParseIdlOpts::default()
    };
    let idl = shank_idl::extract_idl(lib_path_str, opts)
        .with_context(|| "Failed to extract IDL from program source")?
        .ok_or_else(|| anyhow::anyhow!("No IDL could be extracted from this program"))?;

    // shank_idl only recognizes error enums that derive `thiserror::Error`. If
    // it found none, fall back to detecting a plain enum with a manual
    // `impl From<X> for ProgramError` instead of silently emitting no errors.
    let errors = if idl.errors.as_ref().map_or(true, |e| e.is_empty()) {
        let src_dir = lib_path.parent().unwrap_or(&crate_root);
        manual_errors::find_manual_program_errors(src_dir)?
    } else {
        None
    };
    if let Some(errors) = &errors {
        if !errors.is_empty() {
            println!("ℹ️  No thiserror-derived errors found; detected {} error code(s) via a manual `impl From<_> for ProgramError`", errors.len());
        }
    }

    let idl_json = render_idl_json(&idl, errors.as_deref())
        .with_context(|| "Failed to serialize IDL to JSON")?;

    let codama_idl = codama::to_codama_compatible(&idl);
    let codama_idl_json = render_idl_json(&codama_idl, errors.as_deref())
        .with_context(|| "Failed to serialize codama-compatible IDL to JSON")?;

    let out_path = Path::new(out_dir);
    fs::create_dir_all(out_path)
        .with_context(|| format!("Failed to create output directory: {}", out_dir))?;
    let lib_name = manifest.lib_name()?;
    let idl_file = out_path.join(format!("{lib_name}.json"));
    let codama_idl_file = out_path.join(format!("{lib_name}.codama.json"));
    fs::write(&idl_file, idl_json).with_context(|| format!("Failed to write {}", idl_file.display()))?;
    fs::write(&codama_idl_file, codama_idl_json)
        .with_context(|| format!("Failed to write {}", codama_idl_file.display()))?;

    println!("✅ IDL written to {}", idl_file.display());
    println!("✅ Codama-compatible IDL written to {}", codama_idl_file.display());
    Ok(())
}

/// Serializes `idl` to pretty JSON, overlaying `manual_errors` onto the
/// `errors` field when shank_idl found none on its own.
fn render_idl_json(
    idl: &shank_idl::idl::Idl,
    errors: Option<&[manual_errors::ManualErrorCode]>,
) -> Result<String> {
    let mut value = serde_json::to_value(idl)?;
    if let Some(errors) = errors {
        if !errors.is_empty() {
            value["errors"] = serde_json::to_value(errors)?;
        }
    }
    Ok(serde_json::to_string_pretty(&value)?)
}
