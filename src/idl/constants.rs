//! Program constants for the IDL. Neither extractor has a way to declare one
//! from source, so a `const` is exported by a marker comment above it:
//!
//! ```text
//! // pinoc:constant
//! pub const MAX_RESERVE_FLOOR: u64 = 100_000_000 * 1_000_000;
//!
//! // pinoc:constant(u8)
//! pub const MAX_ATAS: usize = 12;
//! ```
//!
//! The value is evaluated from the const's own expression. The type is the
//! declared one, or the marker's when the declared type is not a wire type.

use anyhow::{Context, Result};
use heck::ToLowerCamelCase;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;

const MARKER: &str = "pinoc:constant";
const INTEGER_TYPES: &[&str] = &[
    "u8", "u16", "u32", "u64", "u128", "i8", "i16", "i32", "i64", "i128",
];
/// Codama stores a number value as a JSON number, which JavaScript reads as a
/// double. Beyond this the generated client would see a rounded value.
const MAX_SAFE_INTEGER: i128 = (1 << 53) - 1;

#[derive(Debug, Clone, PartialEq)]
pub struct Constant {
    /// The Rust name, e.g. `MAX_ATAS`.
    pub name: String,
    pub docs: Vec<String>,
    /// An integer type name, e.g. `u8`.
    pub ty: String,
    pub value: i128,
}

impl Constant {
    /// The entry for a shank (Anchor-shaped) IDL's `constants` list.
    pub fn to_shank(&self) -> Value {
        json!({ "name": self.name, "type": self.ty, "value": self.value.to_string() })
    }

    /// A Codama `constantNode`.
    pub fn to_codama(&self) -> Value {
        let mut node = json!({
            "kind": "constantNode",
            "name": self.name.to_lower_camel_case(),
            "type": { "kind": "numberTypeNode", "format": self.ty, "endian": "le" },
            "value": { "kind": "numberValueNode", "number": self.value as i64 },
        });
        if !self.docs.is_empty() {
            node["docs"] = json!(self.docs);
        }
        node
    }
}

/// Every `const` under `src_dir` that carries the marker, in file then source order.
pub fn find_constants(src_dir: &Path) -> Result<Vec<Constant>> {
    let mut files = Vec::new();
    collect_files(src_dir, &mut files)?;

    let mut parsed = Vec::new();
    let mut all: HashMap<String, syn::Expr> = HashMap::new();
    for path in files {
        let Ok(src) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(file) = syn::parse_file(&src) else {
            continue;
        };
        let mut consts = Vec::new();
        collect_consts(&file.items, &mut consts);
        for item in &consts {
            all.insert(item.ident.to_string(), (*item.expr).clone());
        }
        parsed.push((path, src, consts));
    }

    let mut out = Vec::new();
    for (path, src, consts) in &parsed {
        let lines: Vec<&str> = src.lines().collect();
        let mut claimed = Vec::new();
        for item in consts {
            let Some((marker_line, type_override)) = marker_above(item, &lines) else {
                continue;
            };
            claimed.push(marker_line);
            let name = item.ident.to_string();
            let at = format!(
                "{}:{}",
                path.display(),
                item.const_token.span().start().line
            );
            let ty = constant_type(item, type_override.as_deref())
                .with_context(|| format!("constant `{name}` at {at}"))?;
            let value = eval(&item.expr, &all, 0).with_context(|| {
                format!("constant `{name}` at {at}: its value is not an integer expression pinoc can evaluate (literals, `+ - * / <<`, casts, and other consts)")
            })?;
            check_range(&ty, value).with_context(|| format!("constant `{name}` at {at}"))?;
            out.push(Constant {
                name,
                docs: doc_lines(&item.attrs),
                ty,
                value,
            });
        }
        // A marker that is not directly above a const exports nothing; say so.
        for (index, line) in lines.iter().enumerate() {
            if marker_of(line).is_some() && !claimed.contains(&index) {
                anyhow::bail!(
                    "`// {MARKER}` at {}:{} is not directly above a `const` item",
                    path.display(),
                    index + 1
                );
            }
        }
    }
    Ok(out)
}

fn collect_consts(items: &[syn::Item], out: &mut Vec<syn::ItemConst>) {
    for item in items {
        match item {
            syn::Item::Const(c) => out.push(c.clone()),
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_consts(inner, out);
                }
            }
            _ => {}
        }
    }
}

/// `Some(type override)` if `line` is a marker comment.
fn marker_of(line: &str) -> Option<Option<String>> {
    let rest = line
        .trim()
        .strip_prefix("//")?
        .trim()
        .strip_prefix(MARKER)?;
    let rest = rest.trim();
    if rest.is_empty() {
        return Some(None);
    }
    let ty = rest.strip_prefix('(')?.split(')').next()?.trim();
    Some(Some(ty.to_string()))
}

/// The marker in the comment block directly above `item` (above its doc
/// comments and attributes, or between them and the `const` keyword).
fn marker_above(item: &syn::ItemConst, lines: &[&str]) -> Option<(usize, Option<String>)> {
    let keyword = item.const_token.span().start().line.checked_sub(1)?;
    let mut top = item.span().start().line.checked_sub(1)?.min(keyword);
    while top > 0 && lines.get(top - 1)?.trim().starts_with("//") {
        top -= 1;
    }
    (top..keyword).find_map(|index| Some((index, marker_of(lines.get(index)?)?)))
}

fn constant_type(item: &syn::ItemConst, type_override: Option<&str>) -> Result<String> {
    let declared = match item.ty.as_ref() {
        syn::Type::Path(p) => p.path.get_ident().map(|i| i.to_string()),
        _ => None,
    };
    let chosen = type_override.map(str::to_string).or(declared.clone());
    match chosen {
        Some(ty) if INTEGER_TYPES.contains(&ty.as_str()) => Ok(ty),
        Some(ty) if type_override.is_some() => anyhow::bail!(
            "`// {MARKER}({ty})` is not an integer type; use one of {}",
            INTEGER_TYPES.join(", ")
        ),
        _ => anyhow::bail!(
            "its type `{}` is not a fixed-width integer, so it has no wire type; state one in the marker, e.g. `// {MARKER}(u8)`",
            declared.unwrap_or_else(|| "?".to_string())
        ),
    }
}

fn check_range(ty: &str, value: i128) -> Result<()> {
    let bits: u32 = ty[1..].parse()?;
    let (min, max) = if ty.starts_with('u') {
        (
            0,
            if bits == 128 {
                i128::MAX
            } else {
                (1i128 << bits) - 1
            },
        )
    } else if bits == 128 {
        (i128::MIN, i128::MAX)
    } else {
        (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
    };
    if value < min || value > max {
        anyhow::bail!("value {value} does not fit in `{ty}`");
    }
    if value.abs() > MAX_SAFE_INTEGER {
        anyhow::bail!(
            "value {value} is beyond 2^53, which an IDL number cannot carry exactly; a JavaScript client would read a rounded value"
        );
    }
    Ok(())
}

fn eval(expr: &syn::Expr, consts: &HashMap<String, syn::Expr>, depth: usize) -> Result<i128> {
    if depth > 16 {
        anyhow::bail!("the expression refers to itself");
    }
    let next = |e: &syn::Expr| eval(e, consts, depth + 1);
    match expr {
        syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(int),
            ..
        }) => Ok(int.base10_parse::<i128>()?),
        syn::Expr::Paren(e) => next(&e.expr),
        syn::Expr::Group(e) => next(&e.expr),
        syn::Expr::Cast(e) => next(&e.expr),
        syn::Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Neg(_),
            expr,
            ..
        }) => next(expr)?
            .checked_neg()
            .ok_or_else(|| anyhow::anyhow!("overflow")),
        syn::Expr::Binary(bin) => {
            let (left, right) = (next(&bin.left)?, next(&bin.right)?);
            let result = match bin.op {
                syn::BinOp::Add(_) => left.checked_add(right),
                syn::BinOp::Sub(_) => left.checked_sub(right),
                syn::BinOp::Mul(_) => left.checked_mul(right),
                syn::BinOp::Div(_) => left.checked_div(right),
                syn::BinOp::Shl(_) => u32::try_from(right).ok().and_then(|r| left.checked_shl(r)),
                _ => anyhow::bail!("unsupported operator"),
            };
            result.ok_or_else(|| anyhow::anyhow!("overflow"))
        }
        syn::Expr::Path(path) => {
            let name = path
                .path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            let referenced = consts
                .get(&name)
                .ok_or_else(|| anyhow::anyhow!("`{name}` is not a const in this crate"))?;
            next(referenced)
        }
        _ => anyhow::bail!("unsupported expression"),
    }
}

fn doc_lines(attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("doc"))
        .filter_map(|a| match &a.meta {
            syn::Meta::NameValue(nv) => match &nv.value {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) => Some(s.value().trim().to_string()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    out.sort();
    Ok(())
}
