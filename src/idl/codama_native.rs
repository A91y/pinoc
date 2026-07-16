//! Detects Codama's own Rust derive macros (`CodamaAccount`, `CodamaInstructions`,
//! etc.) and, when present, extracts a native Codama IDL directly instead of
//! going through the shank IDL + compatibility-shim path.

use anyhow::{Context, Result};
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
        paths
            .iter()
            .any(|p| CODAMA_DERIVES.iter().any(|name| p.is_ident(name)))
    })
}

/// Runs Codama's extractor and injects `resolved_address`, since Codama's own
/// `declare_id!` detection doesn't recognize Pinocchio's macro path.
pub fn extract_native_codama_idl(
    crate_root: &Path,
    resolved_address: Option<&str>,
) -> Result<String> {
    let json = codama::Codama::load(crate_root)?.get_json_idl()?;
    let mut value: Value = serde_json::from_str(&json)?;
    if let Some(address) = resolved_address {
        value["program"]["publicKey"] = Value::String(address.to_string());
    }

    // Codama::load() doesn't error on a crate with no Codama macros at all,
    // it just returns an empty program, which is easy to mistake for success.
    if program_is_empty(&value) {
        println!("⚠️  Native Codama extraction found no instructions, accounts, or errors. Does this program actually use Codama's derive macros?");
    }

    Ok(serde_json::to_string_pretty(&value)?)
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
