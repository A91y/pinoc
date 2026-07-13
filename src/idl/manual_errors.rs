//! Fallback used when `shank_idl` finds no errors, since it only recognizes
//! enums deriving `thiserror::Error`: scans `src/` for an enum manually
//! `impl From<X> for ProgramError`'d instead, synthesizing messages from
//! variant names (`NotDone` -> "Not Done").

use anyhow::Result;
use serde::Serialize;
use std::path::Path;
use syn::{Expr, ExprLit, GenericArgument, Item, ItemEnum, ItemImpl, Lit, PathArguments, Type};

#[derive(Debug, Clone, Serialize)]
pub struct ManualErrorCode {
    pub code: u32,
    pub name: String,
    pub msg: Option<String>,
}

/// Scans `src_dir` for a manually-implemented `ProgramError` source enum and
/// returns its variants as error codes, or `None` if no such enum is found.
pub fn find_manual_program_errors(src_dir: &Path) -> Result<Option<Vec<ManualErrorCode>>> {
    let mut enums: Vec<ItemEnum> = Vec::new();
    let mut program_error_targets: Vec<String> = Vec::new();
    collect_items(src_dir, &mut enums, &mut program_error_targets)?;

    let Some(enum_item) = enums
        .into_iter()
        .find(|e| program_error_targets.contains(&e.ident.to_string()))
    else {
        return Ok(None);
    };

    let mut errors = Vec::new();
    let mut next_code: u32 = 0;
    for variant in &enum_item.variants {
        if let Some((_, expr)) = &variant.discriminant {
            if let Some(code) = literal_u32(expr) {
                next_code = code;
            }
        }
        let name = variant.ident.to_string();
        errors.push(ManualErrorCode {
            code: next_code,
            msg: Some(pascal_case_to_words(&name)),
            name,
        });
        next_code += 1;
    }

    Ok(Some(errors))
}

fn collect_items(
    dir: &Path,
    enums: &mut Vec<ItemEnum>,
    program_error_targets: &mut Vec<String>,
) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_items(&path, enums, program_error_targets)?;
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
        for item in file.items {
            match item {
                Item::Enum(item_enum) => enums.push(item_enum),
                Item::Impl(item_impl) => {
                    if let Some(target) = manual_program_error_target(&item_impl) {
                        program_error_targets.push(target);
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Returns the enum type name `X` if `item_impl` is `impl From<X> for
/// <something ending in ProgramError>`.
fn manual_program_error_target(item_impl: &ItemImpl) -> Option<String> {
    let (_, trait_path, _) = item_impl.trait_.as_ref()?;
    let trait_segment = trait_path.segments.last()?;
    if trait_segment.ident != "From" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &trait_segment.arguments else {
        return None;
    };
    let GenericArgument::Type(Type::Path(from_type_path)) = args.args.first()? else {
        return None;
    };
    let from_ty = from_type_path.path.segments.last()?.ident.to_string();

    let Type::Path(self_type_path) = item_impl.self_ty.as_ref() else {
        return None;
    };
    if self_type_path.path.segments.last()?.ident != "ProgramError" {
        return None;
    }

    Some(from_ty)
}

fn literal_u32(expr: &Expr) -> Option<u32> {
    match expr {
        Expr::Lit(ExprLit { lit: Lit::Int(lit_int), .. }) => lit_int.base10_parse::<u32>().ok(),
        _ => None,
    }
}

fn pascal_case_to_words(name: &str) -> String {
    let mut result = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_uppercase() && i != 0 {
            result.push(' ');
        }
        result.push(ch);
    }
    result
}
