//! Enums whose variants carry explicit discriminants that differ from their
//! positions (`Open = 1, Closed = 5`). Every renderer pinoc drives encodes an
//! enum by variant position, so such an enum would be written and read as the
//! wrong byte with no error. This module finds those enums and rewrites the
//! rendered clients to use the declared values, failing when it cannot.

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// A unit-only enum and the value of each variant, in declaration order.
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDiscriminants {
    pub name: String,
    pub values: Vec<u64>,
}

/// Lowercased alphanumerics, to pair a name across the Rust source, the IDL,
/// and each renderer's own casing.
fn key(name: &str) -> String {
    name.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn is_positional(values: &[u64]) -> bool {
    values.iter().enumerate().all(|(i, v)| *v == i as u64)
}

pub fn find<'a>(enums: &'a [EnumDiscriminants], name: &str) -> Option<&'a EnumDiscriminants> {
    let wanted = key(name);
    enums.iter().find(|e| key(&e.name) == wanted)
}

/// The enums to rewrite for the IDL at `idl_path`: read from the IDL when it
/// is a native Codama document, else from the program source, since the shank
/// IDL does not record discriminants.
pub fn for_idl(idl_path: &Path, src_dir: &Path) -> Result<Vec<EnumDiscriminants>> {
    let idl: Value = serde_json::from_str(
        &std::fs::read_to_string(idl_path)
            .with_context(|| format!("Failed to read {}", idl_path.display()))?,
    )
    .with_context(|| format!("Failed to parse {}", idl_path.display()))?;
    if idl["kind"] == "rootNode" {
        return from_codama_idl(&idl);
    }
    let idl_enums: Vec<String> = idl["types"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|t| t["type"]["kind"] == "enum")
        .filter_map(|t| t["name"].as_str().map(key))
        .collect();
    Ok(from_source(src_dir)?
        .into_iter()
        .filter(|e| idl_enums.contains(&key(&e.name)))
        .collect())
}

/// Unit-only enums under `src_dir` whose explicit discriminants are not `0..n`.
pub fn from_source(src_dir: &Path) -> Result<Vec<EnumDiscriminants>> {
    let mut files = Vec::new();
    collect_files(src_dir, "rs", &mut files)?;
    let mut out = Vec::new();
    for path in files {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(file) = syn::parse_file(&src) else {
            continue;
        };
        collect_enums(&file.items, &mut out);
    }
    Ok(out)
}

fn collect_enums(items: &[syn::Item], out: &mut Vec<EnumDiscriminants>) {
    for item in items {
        match item {
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_enums(inner, out);
                }
            }
            syn::Item::Enum(e) => {
                if !e
                    .variants
                    .iter()
                    .all(|v| matches!(v.fields, syn::Fields::Unit))
                {
                    continue;
                }
                let mut values = Vec::new();
                let mut next = Some(0u64);
                for variant in &e.variants {
                    if let Some((_, expr)) = &variant.discriminant {
                        next = literal_u64(expr);
                    }
                    let Some(value) = next else {
                        break;
                    };
                    values.push(value);
                    next = value.checked_add(1);
                }
                // A discriminant that is not an integer literal leaves the values unknown.
                if values.len() == e.variants.len() && !is_positional(&values) {
                    out.push(EnumDiscriminants {
                        name: e.ident.to_string(),
                        values,
                    });
                }
            }
            _ => {}
        }
    }
}

fn literal_u64(expr: &syn::Expr) -> Option<u64> {
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(int),
            ..
        }) => int.base10_parse().ok(),
        syn::Expr::Paren(p) => literal_u64(&p.expr),
        syn::Expr::Group(g) => literal_u64(&g.expr),
        _ => None,
    }
}

/// Enums in a native Codama IDL whose discriminators are not their positions.
/// Errors on the shapes the rendered clients cannot be corrected for.
pub fn from_codama_idl(idl: &Value) -> Result<Vec<EnumDiscriminants>> {
    let mut out = Vec::new();
    walk_codama(idl, None, &mut out)?;
    Ok(out)
}

fn walk_codama(node: &Value, named: Option<&str>, out: &mut Vec<EnumDiscriminants>) -> Result<()> {
    match node {
        Value::Array(items) => items.iter().try_for_each(|n| walk_codama(n, None, out)),
        Value::Object(map) => {
            if map.get("kind").and_then(Value::as_str) == Some("enumTypeNode") {
                record_codama_enum(node, named, out)?;
            }
            for (field, child) in map {
                // Only the direct `type` of a defined type is that named enum.
                let name = (map.get("kind").and_then(Value::as_str) == Some("definedTypeNode")
                    && field == "type")
                    .then(|| map.get("name").and_then(Value::as_str))
                    .flatten();
                walk_codama(child, name, out)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn record_codama_enum(
    node: &Value,
    named: Option<&str>,
    out: &mut Vec<EnumDiscriminants>,
) -> Result<()> {
    let variants = node["variants"].as_array().cloned().unwrap_or_default();
    let mut values = Vec::new();
    let mut next = 0u64;
    for variant in &variants {
        let value = variant["discriminator"].as_u64().unwrap_or(next);
        values.push(value);
        next = value.saturating_add(1);
    }
    if is_positional(&values) {
        return Ok(());
    }
    let listed = values
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let Some(name) = named else {
        anyhow::bail!(
            "an enum declared inline in the IDL has explicit discriminators ({listed}) that are not its variant positions. The generated client would encode it by position. Declare it as its own type (`#[derive(CodamaType)]`) so pinoc can generate it with its values."
        );
    };
    if variants
        .iter()
        .any(|v| v["kind"] != "enumEmptyVariantTypeNode")
    {
        anyhow::bail!(
            "enum `{name}` has variants with data and explicit discriminators ({listed}) that are not their positions. The generated client would encode it by position, and pinoc cannot correct an enum with data. Use discriminators 0..n, or keep this enum out of the generated client."
        );
    }
    out.push(EnumDiscriminants {
        name: name.to_string(),
        values,
    });
    Ok(())
}

/// Rewrites the rendered Rust enums to their declared values. `borsh` then
/// writes the value, with `use_discriminant`, where it wrote the position.
pub fn patch_rust(generated_dir: &Path, enums: &[EnumDiscriminants]) -> Result<()> {
    for e in enums {
        patch_enum(generated_dir, "rs", "pub enum ", e, |text, name| {
            let declaration = format!("pub enum {name} ");
            let already = text
                .lines()
                .take_while(|line| !line.trim_start().starts_with(&declaration))
                .collect::<Vec<_>>()
                .iter()
                .rev()
                .take_while(|line| line.trim_start().starts_with("#["))
                .any(|line| line.contains("use_discriminant"));
            if already {
                return Ok(text);
            }
            Ok(text.replacen(
                &declaration,
                &format!("#[borsh(use_discriminant = true)]\n{declaration}"),
                1,
            ))
        })?;
    }
    Ok(())
}

/// Rewrites the rendered TypeScript enums to their declared values and makes
/// their codecs use those values; explicit values alone still encode by position.
pub fn patch_ts(generated_dir: &Path, enums: &[EnumDiscriminants]) -> Result<()> {
    for e in enums {
        patch_enum(generated_dir, "ts", "export enum ", e, |text, name| {
            let mut text = text;
            for codec in ["getEnumEncoder", "getEnumDecoder"] {
                let positional = format!("{codec}({name})");
                if text.contains(&positional) {
                    text = text.replace(
                        &positional,
                        &format!("{codec}({name}, {{ useValuesAsDiscriminators: true }})"),
                    );
                    continue;
                }
                // A renderer that already passes the option needs no change.
                let with_options = text
                    .split(&format!("{codec}({name},"))
                    .nth(1)
                    .and_then(|rest| rest.split(')').next())
                    .is_some_and(|options| options.contains("useValuesAsDiscriminators: true"));
                if !with_options {
                    anyhow::bail!("`{positional}` not found");
                }
            }
            Ok(text)
        })?;
    }
    Ok(())
}

/// Finds the file declaring `e`, gives each variant its value by position, and
/// applies `finish` (which receives the text and the enum's rendered name).
/// Any departure from the expected text is an error: a client that silently
/// encodes the wrong value must not be left looking finished.
fn patch_enum(
    dir: &Path,
    extension: &str,
    declaration: &str,
    e: &EnumDiscriminants,
    finish: impl Fn(String, &str) -> Result<String>,
) -> Result<()> {
    let mut files = Vec::new();
    collect_files(dir, extension, &mut files)?;
    let fail = |why: String| {
        anyhow::anyhow!(
            "enum `{}` has explicit discriminants that the renderer does not emit, and pinoc could not rewrite the generated code ({why}). The client in {} encodes this enum by variant position; do not use it.",
            e.name,
            dir.display()
        )
    };

    for path in files {
        let text = std::fs::read_to_string(&path)?;
        let lines: Vec<&str> = text.lines().collect();
        let Some((start, rendered_name)) = lines.iter().enumerate().find_map(|(i, line)| {
            let name = line
                .trim()
                .strip_prefix(declaration)?
                .strip_suffix('{')?
                .trim();
            (key(name) == key(&e.name)).then(|| (i, name.to_string()))
        }) else {
            continue;
        };
        let end = lines[start..]
            .iter()
            .position(|line| line.trim() == "}")
            .map(|offset| start + offset)
            .ok_or_else(|| fail("no closing brace".to_string()))?;

        let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        let mut values = e.values.iter();
        for line in &mut out[start + 1..end] {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || ["//", "#", "/*", "*"]
                    .iter()
                    .any(|p| trimmed.starts_with(p))
            {
                continue;
            }
            let entry = trimmed.strip_suffix(',').unwrap_or(trimmed);
            let (variant, rendered_value) = match entry.split_once('=') {
                Some((variant, value)) => (variant.trim(), Some(value.trim())),
                None => (entry, None),
            };
            let is_ident = !variant.is_empty()
                && variant
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_');
            let (true, Some(value)) = (is_ident, values.next()) else {
                return Err(fail(format!("unexpected variant line `{trimmed}`")));
            };
            // A renderer that emits the values itself must agree with the program.
            if rendered_value.is_some_and(|v| v.parse() != Ok(*value)) {
                return Err(fail(format!(
                    "variant line `{trimmed}` does not carry the declared value {value}"
                )));
            }
            let indent = &line[..line.len() - line.trim_start().len()];
            *line = format!("{indent}{variant} = {value},");
        }
        if values.next().is_some() {
            return Err(fail("fewer variants rendered than declared".to_string()));
        }

        let mut patched = out.join("\n");
        if text.ends_with('\n') {
            patched.push('\n');
        }
        let patched = finish(patched, &rendered_name).map_err(|err| fail(err.to_string()))?;
        std::fs::write(&path, patched)
            .with_context(|| format!("Failed to write {}", path.display()))?;
        return Ok(());
    }
    Err(fail("its declaration was not found".to_string()))
}

fn collect_files(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) != Some("node_modules") {
                collect_files(&path, extension, out)?;
            }
        } else if path.extension().and_then(|e| e.to_str()) == Some(extension) {
            out.push(path);
        }
    }
    out.sort();
    Ok(())
}

/// The line `pinoc client generate` prints when it applied declared values.
pub fn describe(enums: &[EnumDiscriminants]) -> String {
    let names = enums
        .iter()
        .map(|e| format!("`{}`", e.name))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "ℹ️  Enums with explicit discriminants ({names}) are generated with their declared values, not their variant positions"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(name: &str, file: &str, text: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pinoc-unit-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(file), text).unwrap();
        dir
    }

    fn status() -> Vec<EnumDiscriminants> {
        vec![EnumDiscriminants {
            name: "status".to_string(),
            values: vec![1, 2, 5],
        }]
    }

    const TS: &str = "export enum Status {\n  /** Docs. */\n  Open,\n  Active,\n  Closed,\n}\nconst e = getEnumEncoder(Status);\nconst d = getEnumDecoder(Status);\n";
    const TS_FIXED: &str = "export enum Status {\n  /** Docs. */\n  Open = 1,\n  Active = 2,\n  Closed = 5,\n}\nconst e = getEnumEncoder(Status, { useValuesAsDiscriminators: true });\nconst d = getEnumDecoder(Status, { useValuesAsDiscriminators: true });\n";

    #[test]
    fn typescript_enum_is_rewritten() {
        let dir = dir_with("ts", "status.ts", TS);
        patch_ts(&dir, &status()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("status.ts")).unwrap(),
            TS_FIXED
        );
    }

    #[test]
    fn output_that_already_has_the_values_is_accepted_unchanged() {
        let dir = dir_with("ts-fixed", "status.ts", TS_FIXED);
        patch_ts(&dir, &status()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("status.ts")).unwrap(),
            TS_FIXED
        );

        let rust = "#[derive(Clone)]\n#[borsh(use_discriminant = true)]\npub enum Status {\nOpen = 1,\nActive = 2,\nClosed = 5,\n}\n";
        let dir = dir_with("rs-fixed", "status.rs", rust);
        patch_rust(&dir, &status()).unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join("status.rs")).unwrap(),
            rust
        );
    }

    #[test]
    fn unexpected_output_is_an_error_not_a_guess() {
        for (name, text) in [
            ("fewer", "export enum Status {\n  Open,\n  Active,\n}\ngetEnumEncoder(Status); getEnumDecoder(Status);\n"),
            ("wrong-value", "export enum Status {\n  Open = 0,\n  Active = 2,\n  Closed = 5,\n}\ngetEnumEncoder(Status); getEnumDecoder(Status);\n"),
            ("no-codec", "export enum Status {\n  Open,\n  Active,\n  Closed,\n}\n"),
            ("missing", "export enum Other {\n  A,\n}\n"),
        ] {
            let dir = dir_with(name, "status.ts", text);
            let err = patch_ts(&dir, &status()).unwrap_err().to_string();
            assert!(err.contains("do not use it"), "{name}: {err}");
            assert_eq!(std::fs::read_to_string(dir.join("status.ts")).unwrap(), text, "{name}");
        }
    }

    #[test]
    fn positional_and_data_enums_in_a_codama_idl() {
        let idl = |variants: Value, wrap: bool| {
            let enum_node = serde_json::json!({ "kind": "enumTypeNode", "variants": variants });
            if wrap {
                serde_json::json!({ "kind": "definedTypeNode", "name": "status", "type": enum_node })
            } else {
                serde_json::json!({ "kind": "structFieldTypeNode", "name": "f", "type": enum_node })
            }
        };
        let empty = |d: Option<u64>| match d {
            Some(d) => {
                serde_json::json!({ "kind": "enumEmptyVariantTypeNode", "discriminator": d })
            }
            None => serde_json::json!({ "kind": "enumEmptyVariantTypeNode" }),
        };

        // Positional, written out or not: nothing to do.
        let positional = serde_json::json!([empty(None), empty(Some(1)), empty(None)]);
        assert_eq!(from_codama_idl(&idl(positional, true)).unwrap(), vec![]);

        // A value continues from the previous one, as in Rust.
        let gaps = serde_json::json!([empty(Some(1)), empty(None), empty(Some(5))]);
        assert_eq!(from_codama_idl(&idl(gaps.clone(), true)).unwrap(), status());

        assert!(from_codama_idl(&idl(gaps, false))
            .unwrap_err()
            .to_string()
            .contains("declared inline"));
        let data = serde_json::json!([
            { "kind": "enumStructVariantTypeNode", "discriminator": 3 },
            empty(Some(7)),
        ]);
        assert!(from_codama_idl(&idl(data, true))
            .unwrap_err()
            .to_string()
            .contains("variants with data"));
    }
}
