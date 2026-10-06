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

/// The `codama` release pinoc's own extractor is built from.
pub const BUNDLED_CODAMA: &str = "0.9.3";

/// Either crate provides the derives. `codama-macros` is the one a program
/// using `nostd_panic_handler!` can depend on, since the `codama` facade links `std`.
const CODAMA_CRATES: &[&str] = &["codama", "codama-macros"];

/// How a program uses Codama's derive macros.
pub struct CodamaUsage {
    /// The Codama crate the manifest depends on, if any.
    pub dependency: Option<String>,
    /// At least one source file derives a Codama macro.
    pub derives: bool,
}

impl CodamaUsage {
    pub fn detected(&self) -> bool {
        self.dependency.is_some() && self.derives
    }

    /// Why Codama was not detected, for the messages that say so.
    pub fn undetected_reason(&self) -> &'static str {
        if self.derives && self.dependency.is_none() {
            "Codama derives found, but neither `codama` nor `codama-macros` is a dependency"
        } else {
            "no Codama macros detected"
        }
    }
}

pub fn codama_usage(crate_root: &Path, src_dir: &Path) -> Result<CodamaUsage> {
    let cargo_toml = crate_root.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml)
        .with_context(|| format!("Failed to read {}", cargo_toml.display()))?;
    let manifest: toml::Value = toml::from_str(&content)
        .with_context(|| format!("Failed to parse {}", cargo_toml.display()))?;
    Ok(CodamaUsage {
        dependency: codama_dependency(&manifest).map(|(name, _)| name),
        derives: scan_for_codama_derives(src_dir)?,
    })
}

/// True if the program depends on `codama` or `codama-macros` and at least one
/// file under `src_dir` derives one of Codama's macros.
pub fn codama_macros_detected(crate_root: &Path, src_dir: &Path) -> Result<bool> {
    Ok(codama_usage(crate_root, src_dir)?.detected())
}

/// The Codama crate in `[dependencies]` or any `[target.<cfg>.dependencies]`,
/// with its declared version requirement. A renamed dependency is matched on
/// its `package`.
fn codama_dependency(manifest: &toml::Value) -> Option<(String, Option<String>)> {
    let targets = manifest
        .get("target")
        .and_then(|t| t.as_table())
        .into_iter()
        .flat_map(|t| t.values());
    std::iter::once(manifest)
        .chain(targets)
        .filter_map(|section| section.get("dependencies")?.as_table())
        .flatten()
        .find_map(|(key, spec)| {
            let name = spec.get("package").and_then(|p| p.as_str()).unwrap_or(key);
            if !CODAMA_CRATES.contains(&name) {
                return None;
            }
            let version = spec.as_str().or_else(|| spec.get("version")?.as_str());
            Some((name.to_string(), version.map(str::to_string)))
        })
}

/// A note for a program whose Codama macros are from a different minor release
/// than pinoc's extractor, or `None` when they match or the version is unknown.
pub fn version_mismatch(crate_root: &Path) -> Option<String> {
    let version = program_codama_version(crate_root)?;
    (minor_of(&version)? != minor_of(BUNDLED_CODAMA)?).then(|| {
        format!(
            "this program uses codama-macros {version}, and pinoc extracts with codama {BUNDLED_CODAMA}. The `#[codama(..)]` directives differ between releases; one that {BUNDLED_CODAMA} does not recognise stops the extraction"
        )
    })
}

/// The `codama-macros` version the program resolves to: from the nearest
/// `Cargo.lock` (the facade depends on the macros, so it is listed either
/// way), else the requirement in the manifest.
fn program_codama_version(crate_root: &Path) -> Option<String> {
    let locked = crate_root.ancestors().take(4).find_map(|dir| {
        let lock: toml::Value =
            toml::from_str(&std::fs::read_to_string(dir.join("Cargo.lock")).ok()?).ok()?;
        let versions: Vec<String> = lock
            .get("package")?
            .as_array()?
            .iter()
            .filter(|p| p.get("name").and_then(|n| n.as_str()) == Some("codama-macros"))
            .filter_map(|p| Some(p.get("version")?.as_str()?.to_string()))
            .collect();
        // With several in the lock file, report one that differs.
        versions
            .iter()
            .find(|v| minor_of(v) != minor_of(BUNDLED_CODAMA))
            .or(versions.first())
            .cloned()
    });
    locked.or_else(|| {
        let manifest: toml::Value =
            toml::from_str(&std::fs::read_to_string(crate_root.join("Cargo.toml")).ok()?).ok()?;
        let requirement = codama_dependency(&manifest)?.1?;
        Some(
            requirement
                .trim_start_matches(|c: char| !c.is_ascii_digit())
                .to_string(),
        )
    })
}

/// `(major, minor)` of a version or requirement such as `0.13.2` or `0.9`.
fn minor_of(version: &str) -> Option<(u64, u64)> {
    let mut parts = version.split('.');
    let major = parts.next()?.trim().parse().ok()?;
    let minor = parts
        .next()
        .and_then(|m| m.trim().parse().ok())
        .unwrap_or(0);
    Some((major, minor))
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
    src_dir: &Path,
    resolved_address: Option<&str>,
    fallback_errors: &[Value],
    manual: Option<&ManualErrors>,
) -> Result<String> {
    let json = codama::Codama::load(crate_root)
        .and_then(|codama| codama.get_json_idl())
        .map_err(|e| extraction_error(e, crate_root, src_dir))?;
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

/// Codama reports a rejected attribute as a `syn::Error`, which has a span but
/// no file, and prints it as its own source. Flattens it to one message per
/// problem, each with the file and line the span points at.
fn extraction_error(
    error: codama::CodamaError,
    crate_root: &Path,
    src_dir: &Path,
) -> anyhow::Error {
    let codama::CodamaError::Compilation(syn_error) = error else {
        return anyhow::anyhow!("{error}");
    };
    let mut files = Vec::new();
    let _ = collect_sources(src_dir, &mut files);
    for (path, _) in &mut files {
        if let Ok(relative) = path.strip_prefix(crate_root) {
            *path = relative.to_path_buf();
        }
    }
    let problems: Vec<String> = syn_error
        .into_iter()
        .map(|e| {
            let start = e.span().start();
            let token = e.span().source_text().unwrap_or_default();
            let token_note = if token.is_empty() {
                String::new()
            } else {
                format!(" `{token}`")
            };
            // A span carries no file: find the sources that have this token at this position.
            let matches: Vec<&(std::path::PathBuf, String)> = files
                .iter()
                .filter(|(_, src)| {
                    !token.is_empty()
                        && src
                            .lines()
                            .nth(start.line.saturating_sub(1))
                            .and_then(|line| line.get(char_to_byte(line, start.column)..))
                            .is_some_and(|rest| rest.starts_with(&token))
                })
                .collect();
            match matches.as_slice() {
                [(path, src)] => format!(
                    "{e}{token_note}\n  --> {}:{}:{}\n   |  {}",
                    path.display(),
                    start.line,
                    start.column + 1,
                    src.lines().nth(start.line - 1).unwrap_or_default().trim()
                ),
                [] => format!(
                    "{e}{token_note} (line {}, column {})",
                    start.line,
                    start.column + 1
                ),
                several => format!(
                    "{e}{token_note} at line {}, column {} of one of: {}",
                    start.line,
                    start.column + 1,
                    several
                        .iter()
                        .map(|(path, _)| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            }
        })
        .collect();
    anyhow::anyhow!(problems.join("\n"))
}

/// Byte offset of the `column`-th character of `line` (spans count characters).
fn char_to_byte(line: &str, column: usize) -> usize {
    line.char_indices()
        .nth(column)
        .map(|(i, _)| i)
        .unwrap_or(line.len())
}

fn collect_sources(dir: &Path, out: &mut Vec<(std::path::PathBuf, String)>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_sources(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(src) = std::fs::read_to_string(&path) {
                out.push((path, src));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(())
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
