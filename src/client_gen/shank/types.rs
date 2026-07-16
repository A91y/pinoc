use anyhow::Result;
use heck::{ToPascalCase, ToSnakeCase};
use shank_idl::idl_type::IdlType;
use shank_idl::idl_type_definition::{IdlTypeDefinition, IdlTypeDefinitionTy};
use shank_idl::idl_variant::EnumFields;

pub fn idl_type_to_rust(ty: &IdlType) -> Result<String> {
    Ok(match ty {
        IdlType::U8 => "u8".to_string(),
        IdlType::U16 => "u16".to_string(),
        IdlType::U32 => "u32".to_string(),
        IdlType::U64 => "u64".to_string(),
        IdlType::U128 => "u128".to_string(),
        IdlType::I8 => "i8".to_string(),
        IdlType::I16 => "i16".to_string(),
        IdlType::I32 => "i32".to_string(),
        IdlType::I64 => "i64".to_string(),
        IdlType::I128 => "i128".to_string(),
        IdlType::Bool => "bool".to_string(),
        IdlType::String => "String".to_string(),
        // shank emits `Bytes` (not `Vec<u8>`) specifically for `Vec<u8>` fields.
        IdlType::Bytes => "Vec<u8>".to_string(),
        IdlType::PublicKey => "solana_pubkey::Pubkey".to_string(),
        // `Defined("Address"/"publicKey")` are shank's two ways of saying "pubkey"
        // when it doesn't recognize the type as the literal PublicKey variant.
        IdlType::Defined(name) if name == "Address" || name.eq_ignore_ascii_case("publicKey") => {
            "solana_pubkey::Pubkey".to_string()
        }
        IdlType::Defined(name) => name.to_pascal_case(),
        IdlType::Array(inner, size) => format!("[{}; {size}]", idl_type_to_rust(inner)?),
        IdlType::Vec(inner) => format!("Vec<{}>", idl_type_to_rust(inner)?),
        IdlType::Option(inner) => format!("Option<{}>", idl_type_to_rust(inner)?),
        other => anyhow::bail!("pinoc client generate does not support the IDL type {other:?} yet"),
    })
}

/// Collects PascalCase names of `Defined` types referenced by `ty`, recursing
/// through `Array`/`Vec`/`Option`, excluding the pubkey aliases.
fn collect_defined(ty: &IdlType, out: &mut Vec<String>) {
    match ty {
        IdlType::Defined(name) if name != "Address" && !name.eq_ignore_ascii_case("publicKey") => {
            out.push(name.to_pascal_case());
        }
        IdlType::Array(inner, _) | IdlType::Vec(inner) | IdlType::Option(inner) => {
            collect_defined(inner, out);
        }
        _ => {}
    }
}

/// A `use crate::{...};\n` line importing every `Defined` type referenced by
/// `field_types`, or empty if none. Generated structs live in sibling files, so
/// a field of a defined type needs an explicit import to resolve.
pub fn type_imports<'a>(field_types: impl Iterator<Item = &'a IdlType>) -> String {
    let mut refs = Vec::new();
    for ty in field_types {
        collect_defined(ty, &mut refs);
    }
    refs.sort();
    refs.dedup();
    if refs.is_empty() {
        String::new()
    } else {
        format!("use crate::{{{}}};\n", refs.join(", "))
    }
}

/// Wraps `name` as a raw identifier (`r#name`) when it collides with a Rust
/// keyword, so generated field/param/fn/module names always compile. The few
/// keywords that can't be raw identifiers get a trailing underscore instead.
pub fn safe_ident(name: &str) -> String {
    const NON_RAW: &[&str] = &["crate", "self", "Self", "super", "_"];
    if NON_RAW.contains(&name) {
        return format!("{name}_");
    }
    if is_rust_keyword(name) {
        return format!("r#{name}");
    }
    name.to_string()
}

fn is_rust_keyword(name: &str) -> bool {
    matches!(
        name,
        "as" | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "async"
            | "await"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
            | "try"
    )
}

/// Renders the `pub struct`/`pub enum` definition for a defined type or account,
/// plus the `use crate::{...}` line importing any defined types it references.
/// Borsh's derive gives enums a `u8` variant index then the variant's fields,
/// matching Codama's renderer.
pub fn render_type_body(name: &str, ty: &IdlTypeDefinitionTy) -> Result<(String, String)> {
    match ty {
        IdlTypeDefinitionTy::Struct { fields } => {
            let mut field_lines = String::new();
            for field in fields {
                let field_name = safe_ident(&field.name.to_snake_case());
                let field_ty = idl_type_to_rust(&field.ty)?;
                field_lines.push_str(&format!("    pub {field_name}: {field_ty},\n"));
            }
            let imports = type_imports(fields.iter().map(|f| &f.ty));
            Ok((format!("pub struct {name} {{\n{field_lines}}}\n"), imports))
        }
        IdlTypeDefinitionTy::Enum { variants } => {
            let mut variant_lines = String::new();
            let mut referenced = Vec::new();
            for variant in variants {
                let vname = safe_ident(&variant.name.to_pascal_case());
                match &variant.fields {
                    None => variant_lines.push_str(&format!("    {vname},\n")),
                    Some(EnumFields::Tuple(types)) => {
                        let rendered = types
                            .iter()
                            .map(idl_type_to_rust)
                            .collect::<Result<Vec<_>>>()?;
                        referenced.extend(types.iter().cloned());
                        variant_lines.push_str(&format!("    {vname}({}),\n", rendered.join(", ")));
                    }
                    Some(EnumFields::Named(fields)) => {
                        let mut inner = Vec::new();
                        for field in fields {
                            let field_name = safe_ident(&field.name.to_snake_case());
                            inner.push(format!("{field_name}: {}", idl_type_to_rust(&field.ty)?));
                            referenced.push(field.ty.clone());
                        }
                        variant_lines
                            .push_str(&format!("    {vname} {{ {} }},\n", inner.join(", ")));
                    }
                }
            }
            let imports = type_imports(referenced.iter());
            Ok((format!("pub enum {name} {{\n{variant_lines}}}\n"), imports))
        }
    }
}

pub fn type_def_rs(ty_def: &IdlTypeDefinition) -> Result<String> {
    let name = ty_def.name.to_pascal_case();
    let (body, imports) = render_type_body(&name, &ty_def.ty)?;

    Ok(format!(
        r#"//! Autogenerated by `pinoc client generate`. Do not edit by hand.

use borsh::{{BorshDeserialize, BorshSerialize}};
{imports}
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq)]
{body}"#
    ))
}
