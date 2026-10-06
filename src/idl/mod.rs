//! IDL generation. Always extracts via `shank_idl` for the plain `<name>.json`
//! and the error list (falling back to `manual_errors` if none are found).
//! For `<name>.codama.json` specifically, forks between the `codama` shim
//! (rewrites shank's output) and `codama_native` (Codama's own extractor),
//! per `resolve_generator`'s CLI/Pinoc.toml/detection precedence.

pub mod codama;
pub mod codama_native;
pub mod constants;
pub mod manual_errors;
pub mod padding_lint;

use crate::config;
use anyhow::{Context, Result};
use manual_errors::{Conversion, ManualErrorCode, ManualErrors};
use std::fs;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Generator {
    Shank,
    Codama,
}

pub fn generate_idl(
    out_dir: &str,
    program_id: Option<&str>,
    generator_override: Option<Generator>,
) -> Result<()> {
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
    let src_dir = lib_path.parent().unwrap_or(&crate_root);
    let shank_errors = idl.errors.as_deref().unwrap_or_default();
    let manual = manual_errors::find_manual_program_errors(src_dir)?;
    let errors = match &manual {
        Some(manual) if shank_errors.is_empty() => {
            if !manual.errors.is_empty() {
                println!("ℹ️  No thiserror-derived errors found; detected {} error code(s) via a manual `impl From<_> for ProgramError`", manual.errors.len());
                report_conversion(manual);
            }
            Some(manual.errors.clone())
        }
        // shank reads raw discriminants as well, so apply the same conversion
        // when its errors come from the enum behind the `From` impl.
        Some(manual) => {
            let shank_errors: Vec<ManualErrorCode> =
                serde_json::from_value(serde_json::to_value(shank_errors)?)?;
            match manual_errors::with_converted_codes(shank_errors, manual) {
                Ok(corrected) => {
                    report_conversion(manual);
                    corrected
                }
                Err(variant) => {
                    if manual.conversion != Conversion::Offset(0) {
                        println!("⚠️  Error codes left as raw discriminants: `impl From<_> for ProgramError` converts a different enum than the thiserror-derived one (no variant `{variant}`), so its conversion was not applied. If the program offsets its codes, the IDL codes will not match what it returns.");
                    }
                    None
                }
            }
        }
        None => None,
    };

    let constants = constants::find_constants(src_dir)?;
    if !constants.is_empty() {
        println!(
            "ℹ️  {} constant(s) added to the IDL from `// pinoc:constant` markers",
            constants.len()
        );
    }

    let idl_json = render_idl_json(&idl, errors.as_deref(), &constants)
        .with_context(|| "Failed to serialize IDL to JSON")?;

    let (resolved_generator, forced) = resolve_generator(generator_override, &crate_root, src_dir)?;
    let codama_idl_json = match resolved_generator {
        Generator::Shank => {
            let reason = if forced {
                "forced"
            } else {
                codama_native::codama_usage(&crate_root, src_dir)?.undetected_reason()
            };
            println!("📄 .codama.json: shank IDL + compatibility shim ({reason})");
            let codama_idl = codama::to_codama_compatible(&idl);
            render_idl_json(&codama_idl, errors.as_deref(), &constants)
                .with_context(|| "Failed to serialize codama-compatible IDL to JSON")?
        }
        Generator::Codama => {
            let reason = if forced {
                "forced"
            } else {
                "Codama macros detected"
            };
            println!("🔷 .codama.json: native Codama extraction ({reason})");
            let rendered: serde_json::Value = serde_json::from_str(&idl_json)?;
            let fallback_errors = rendered["errors"].as_array().cloned().unwrap_or_default();
            let mismatch = codama_native::version_mismatch(&crate_root);
            if let Some(note) = &mismatch {
                println!("ℹ️  Codama versions: {note}.");
            }
            codama_native::extract_native_codama_idl(
                &crate_root,
                src_dir,
                idl.metadata.address.as_deref(),
                &fallback_errors,
                manual.as_ref(),
                &constants,
            )
            .with_context(|| match &mismatch {
                Some(note) => format!("Failed to extract native Codama IDL. Note: {note}."),
                None => "Failed to extract native Codama IDL".to_string(),
            })?
        }
    };

    let out_path = Path::new(out_dir);
    fs::create_dir_all(out_path)
        .with_context(|| format!("Failed to create output directory: {}", out_dir))?;
    let lib_name = manifest.lib_name()?;
    let idl_file = out_path.join(format!("{lib_name}.json"));
    let codama_idl_file = out_path.join(format!("{lib_name}.codama.json"));
    fs::write(&idl_file, idl_json)
        .with_context(|| format!("Failed to write {}", idl_file.display()))?;
    fs::write(&codama_idl_file, codama_idl_json)
        .with_context(|| format!("Failed to write {}", codama_idl_file.display()))?;

    println!("✅ IDL written to {}", idl_file.display());
    println!(
        "✅ Codama-compatible IDL written to {}",
        codama_idl_file.display()
    );

    // A #[repr(C)] struct with implicit padding is read zero-copy on-chain but
    // (de)serialized as packed borsh by the client, so the layouts disagree.
    for w in padding_lint::find_padded_repr_c_structs(src_dir)? {
        let pad = w.repr_c_size - w.packed_size;
        println!(
            "⚠️  `{}` is #[repr(C)] with {pad} byte(s) of implicit padding (layout size {} vs packed {}). The generated client (de)serializes it as packed borsh, so it won't round-trip on-chain. Add explicit `_padding: [u8; {pad}]` field(s).",
            w.name, w.repr_c_size, w.packed_size
        );
    }

    Ok(())
}

/// Says how the error codes were derived whenever that is not the plain discriminant.
fn report_conversion(manual: &ManualErrors) {
    match manual.conversion {
        Conversion::Offset(0) => {}
        Conversion::Offset(offset) => println!(
            "ℹ️  Error codes include the {offset:+} offset applied by `impl From<_> for ProgramError`"
        ),
        Conversion::PerVariant => println!(
            "ℹ️  Error codes taken from the `match` in `impl From<_> for ProgramError`"
        ),
        Conversion::Unknown => println!(
            "⚠️  {} error code(s) emitted as raw enum discriminants: the `From` impl was not recognised as a discriminant conversion. If it applies an offset, the IDL codes will not match what the program returns.",
            manual.errors.len()
        ),
    }
}

/// Resolves the `.codama.json` generator: CLI override > `Pinoc.toml` > macro
/// detection. The returned `bool` is `true` when forced, `false` when detected.
fn resolve_generator(
    generator_override: Option<Generator>,
    crate_root: &Path,
    src_dir: &Path,
) -> Result<(Generator, bool)> {
    if let Some(g) = generator_override {
        return Ok((g, true));
    }

    let toml_choice = config::read_pinoc_config_optional()?
        .and_then(|c| c.idl.generator)
        .filter(|s| !s.eq_ignore_ascii_case("auto"));
    if let Some(choice) = toml_choice {
        return match choice.to_ascii_lowercase().as_str() {
            "codama" => Ok((Generator::Codama, true)),
            "shank" => Ok((Generator::Shank, true)),
            _ => anyhow::bail!(
                "Invalid [idl].generator {choice:?} in Pinoc.toml, expected \"auto\", \"shank\", or \"codama\""
            ),
        };
    }

    if codama_native::codama_macros_detected(crate_root, src_dir)? {
        Ok((Generator::Codama, false))
    } else {
        Ok((Generator::Shank, false))
    }
}

/// Serializes `idl` to pretty JSON, overlaying `manual_errors` onto the
/// `errors` field when shank_idl found none on its own.
fn render_idl_json(
    idl: &shank_idl::idl::Idl,
    errors: Option<&[ManualErrorCode]>,
    constants: &[constants::Constant],
) -> Result<String> {
    let mut value = serde_json::to_value(idl)?;
    if let Some(errors) = errors {
        if !errors.is_empty() {
            value["errors"] = serde_json::to_value(errors)?;
        }
    }
    if !constants.is_empty() {
        let mut list = value["constants"].as_array().cloned().unwrap_or_default();
        list.extend(constants.iter().map(constants::Constant::to_shank));
        value["constants"] = serde_json::Value::Array(list);
    }
    Ok(serde_json::to_string_pretty(&value)?)
}
