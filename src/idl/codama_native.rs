//! Detects Codama's own Rust derive macros (`CodamaAccount`, `CodamaInstructions`,
//! etc.) and, when present, extracts a native Codama IDL directly instead of
//! going through the shank IDL + compatibility-shim path.

use super::manual_errors::ManualErrors;
use anyhow::{Context, Result};
use heck::ToLowerCamelCase;
use serde_json::Value;
use std::path::Path;
use syn::{Item, Token};

const CODAMA_DERIVES: &[&str] = &[
    "CodamaAccount",
    "CodamaAccounts",
    "CodamaErrors",
    "CodamaEvent",
    "CodamaEvents",
    "CodamaInstruction",
    "CodamaInstructions",
    "CodamaPda",
    "CodamaType",
];

/// True if `crate_root/Cargo.toml` depends on `codama` and at least one file
/// under `src_dir` derives one of Codama's own macros. The dependency check
/// gates the (more expensive) source walk, since most programs won't have it.
pub fn codama_macros_detected(crate_root: &Path, src_dir: &Path) -> Result<bool> {
    let cargo_toml = crate_root.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml)
        .with_context(|| format!("Failed to read {}", cargo_toml.display()))?;
    let manifest: toml::Value = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", cargo_toml.display()))?;
    let has_codama_dep = manifest
        .get("dependencies")
        .and_then(|deps| deps.get("codama"))
        .is_some();
    if !has_codama_dep {
        return Ok(false);
    }

    scan_for_codama_derives(src_dir)
}

fn scan_for_codama_derives(dir: &Path) -> Result<bool> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            if scan_for_codama_derives(&path)? {
                return Ok(true);
            }
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(file) = syn::parse_file(&src) else {
            continue;
        };
        for item in &file.items {
            if item_has_codama_derive(item) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn item_has_codama_derive(item: &Item) -> bool {
    let attrs = match item {
        Item::Struct(s) => &s.attrs,
        Item::Enum(e) => &e.attrs,
        _ => return false,
    };
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("derive") {
            return false;
        }
        let Ok(paths) = attr
            .parse_args_with(syn::punctuated::Punctuated::<syn::Path, Token![,]>::parse_terminated)
        else {
            return false;
        };
        // Matches `CodamaAccount` and the qualified `codama::CodamaAccount`.
        paths.iter().any(|p| {
            p.segments
                .last()
                .is_some_and(|seg| CODAMA_DERIVES.iter().any(|name| seg.ident == name))
        })
    })
}

/// Runs Codama's extractor and injects `resolved_address`, since Codama's own
/// `declare_id!` detection doesn't recognize Pinocchio's macro path.
///
/// `fallback_errors` is the error list of the shank IDL (`{code, name, msg}`,
/// including the manual-`ProgramError` fallback). It fills `program.errors` only
/// when Codama extracted none, so a program without `CodamaErrors` keeps the
/// errors the shank path reports.
pub fn extract_native_codama_idl(
    crate_root: &Path,
    resolved_address: Option<&str>,
    fallback_errors: &[Value],
    manual: Option<&ManualErrors>,
) -> Result<String> {
    let json = codama::Codama::load(crate_root)?.get_json_idl()?;
    let mut value: Value = serde_json::from_str(&json)?;
    if let Some(address) = resolved_address {
        value["program"]["publicKey"] = Value::String(address.to_string());
    }

    let has_native_errors = value["program"]["errors"]
        .as_array()
        .is_some_and(|a| !a.is_empty());
    let fills_errors = !has_native_errors && !fallback_errors.is_empty();

    // Codama::load() doesn't error on a crate with no Codama macros at all,
    // it just returns an empty program, which is easy to mistake for success.
    if program_is_empty(&value) {
        if fills_errors {
            println!("⚠️  Native Codama extraction found no instructions or accounts (does this program use Codama's derive macros?). Its {} error(s) in .codama.json come from the shank IDL.", fallback_errors.len());
        } else {
            println!("⚠️  Native Codama extraction found no instructions, accounts, or errors. Does this program actually use Codama's derive macros?");
        }
    } else if fills_errors {
        println!(
            "ℹ️  No `CodamaErrors` found; added {} error(s) from the shank IDL to .codama.json",
            fallback_errors.len()
        );
    }

    if has_native_errors {
        backfill_messages(&mut value["program"]["errors"], fallback_errors);
        if let Some(manual) = manual.filter(|m| m.changes_codes()) {
            convert_discriminant_codes(&mut value["program"]["errors"], manual);
        }
    }
    if fills_errors {
        value["program"]["errors"] = Value::Array(fallback_errors.iter().map(error_node).collect());
    }

    Ok(serde_json::to_string_pretty(&value)?)
}

/// Lowercased alphanumerics of a name. Pairs a Rust variant with Codama's
/// camelCase name without depending on how either side splits words
/// (`NotAMint` is `notAmint` to Codama and `notAMint` to heck).
fn name_key(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Codama emits an empty message for an error without a `#[error("..")]`
/// attribute. Fills those from the shank IDL's entry of the same name.
fn backfill_messages(errors: &mut Value, fallback_errors: &[Value]) {
    let Some(nodes) = errors.as_array_mut() else {
        return;
    };
    for node in nodes {
        if !node["message"].as_str().unwrap_or_default().is_empty() {
            continue;
        }
        let key = name_key(node["name"].as_str().unwrap_or_default());
        let fallback = fallback_errors
            .iter()
            .find(|e| name_key(e["name"].as_str().unwrap_or_default()) == key)
            .and_then(|e| e["msg"].as_str());
        if let Some(message) = fallback.filter(|m| !m.is_empty()) {
            node["message"] = Value::from(message);
        }
    }
}

/// Codama takes an error's code from the variant's discriminant unless the
/// program sets one explicitly. Rewrites each code that is still the raw
/// discriminant to what the program's `From` impl returns, and reports every
/// error it had to leave alone.
fn convert_discriminant_codes(errors: &mut Value, manual: &ManualErrors) {
    let Some(nodes) = errors.as_array_mut() else {
        return;
    };
    let mut converted = 0;
    let mut unmatched = Vec::new();
    let mut explicit = Vec::new();
    for node in nodes.iter_mut() {
        let name = node["name"].as_str().unwrap_or_default().to_string();
        let key = name_key(&name);
        let mut matches = manual
            .discriminants
            .iter()
            .enumerate()
            .filter(|(_, (variant, _))| name_key(variant) == key);
        let (Some((index, (_, discriminant))), None) = (matches.next(), matches.next()) else {
            unmatched.push(format!("`{name}`"));
            continue;
        };
        let program_code = manual.errors[index].code;
        let native_code = node["code"].as_u64();
        if native_code == Some(u64::from(program_code)) {
            continue;
        }
        if native_code == Some(u64::from(*discriminant)) {
            node["code"] = Value::from(program_code);
            converted += 1;
        } else {
            explicit.push(format!(
                "`{name}` (Codama {}, program {program_code})",
                node["code"]
            ));
        }
    }

    let total = nodes.len();
    if converted == total {
        println!(
            "ℹ️  .codama.json error codes converted the same way (Codama reads raw discriminants)"
        );
    } else if converted > 0 {
        println!("ℹ️  .codama.json: {converted} of {total} error codes converted the same way (Codama reads raw discriminants)");
    }
    if !unmatched.is_empty() {
        println!(
            "⚠️  .codama.json: {} error code(s) left as raw discriminants, because no variant of the enum behind `impl From<_> for ProgramError` matches: {}. They will not match the codes the program returns.",
            unmatched.len(),
            listed(&unmatched)
        );
    }
    if !explicit.is_empty() {
        println!(
            "⚠️  .codama.json: {} error(s) keep a Codama code that is neither the discriminant nor what `impl From<_> for ProgramError` returns: {}.",
            explicit.len(),
            listed(&explicit)
        );
    }
}

fn listed(names: &[String]) -> String {
    const SHOWN: usize = 5;
    let mut out = names
        .iter()
        .take(SHOWN)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() > SHOWN {
        out.push_str(&format!(", and {} more", names.len() - SHOWN));
    }
    out
}

/// Maps a shank IDL error (`{code, name, msg}`) to a Codama `errorNode`.
fn error_node(error: &Value) -> Value {
    let name = error["name"].as_str().unwrap_or_default();
    serde_json::json!({
        "kind": "errorNode",
        "name": name.to_lower_camel_case(),
        "code": error["code"],
        "message": error["msg"].as_str().unwrap_or_default(),
    })
}

fn program_is_empty(root: &Value) -> bool {
    let is_empty_array = |key: &str| {
        root["program"][key]
            .as_array()
            .map(|a| a.is_empty())
            .unwrap_or(true)
    };
    is_empty_array("instructions") && is_empty_array("accounts") && is_empty_array("errors")
}
