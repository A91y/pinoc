//! Warns when a `#[repr(C)]` struct feeding the IDL (`ShankAccount`/`ShankType`)
//! has implicit alignment padding: it is read zero-copy on-chain but the
//! generated client (de)serializes it as packed borsh, so the layouts disagree.
//! Fix: explicit `_padding: [u8; N]` fields.

use anyhow::Result;
use std::path::Path;
use syn::{Fields, Item, ItemStruct, Type};

pub struct PaddingWarning {
    pub name: String,
    pub packed_size: usize,
    pub repr_c_size: usize,
}

/// Recursively scans `src_dir` for `#[repr(C)]` IDL structs with implicit padding.
pub fn find_padded_repr_c_structs(src_dir: &Path) -> Result<Vec<PaddingWarning>> {
    let mut out = Vec::new();
    scan(src_dir, &mut out)?;
    Ok(out)
}

fn scan(dir: &Path, out: &mut Vec<PaddingWarning>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            scan(&path, out)?;
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
            if let Item::Struct(s) = item {
                if is_repr_c(s) && derives_shank_idl(s) {
                    if let Some(w) = check_padding(s) {
                        out.push(w);
                    }
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn is_repr_c(s: &ItemStruct) -> bool {
    has_repr(s, "C")
}

pub(crate) fn is_repr_transparent(s: &ItemStruct) -> bool {
    has_repr(s, "transparent")
}

fn has_repr(s: &ItemStruct, kind: &str) -> bool {
    s.attrs.iter().any(|a| {
        if !a.path().is_ident("repr") {
            return false;
        }
        let mut found = false;
        let _ = a.parse_nested_meta(|m| {
            if m.path.is_ident(kind) {
                found = true;
            }
            Ok(())
        });
        found
    })
}

pub(crate) fn derives_shank_idl(s: &ItemStruct) -> bool {
    s.attrs.iter().any(|a| {
        if !a.path().is_ident("derive") {
            return false;
        }
        let mut found = false;
        let _ = a.parse_nested_meta(|m| {
            if let Some(seg) = m.path.segments.last() {
                let n = seg.ident.to_string();
                if n == "ShankAccount" || n == "ShankType" {
                    found = true;
                }
            }
            Ok(())
        });
        found
    })
}

/// Returns a warning if `s`'s `#[repr(C)]` layout inserts padding, or `None`
/// (padding-free, or a field type we can't reason about — conservatively quiet).
pub(crate) fn check_padding(s: &ItemStruct) -> Option<PaddingWarning> {
    let Fields::Named(fields) = &s.fields else {
        return None;
    };
    let mut sizes = Vec::new();
    for f in &fields.named {
        sizes.push(size_align(&f.ty)?);
    }
    if sizes.is_empty() {
        return None;
    }

    let packed: usize = sizes.iter().map(|(sz, _)| sz).sum();
    let mut offset = 0usize;
    let mut max_align = 1usize;
    for (sz, al) in &sizes {
        offset = align_up(offset, *al);
        offset += sz;
        max_align = max_align.max(*al);
    }
    let repr_c = align_up(offset, max_align);

    if repr_c != packed {
        Some(PaddingWarning {
            name: s.ident.to_string(),
            packed_size: packed,
            repr_c_size: repr_c,
        })
    } else {
        None
    }
}

fn align_up(offset: usize, align: usize) -> usize {
    offset.div_ceil(align) * align
}

/// `(size, align)` for field types we can reason about; `None` for anything else
/// (`Vec`/`String`/`Option`/nested defined types), which skips the whole struct.
fn size_align(ty: &Type) -> Option<(usize, usize)> {
    match ty {
        Type::Path(p) => {
            let id = p.path.segments.last()?.ident.to_string();
            Some(match id.as_str() {
                "u8" | "i8" | "bool" => (1, 1),
                "u16" | "i16" => (2, 2),
                "u32" | "i32" | "f32" => (4, 4),
                "u64" | "i64" | "f64" => (8, 8),
                "u128" | "i128" => (16, 16),
                // solana_address::Address / solana_pubkey::Pubkey are `[u8; 32]`
                // newtypes: size 32, alignment 1.
                "Address" | "Pubkey" => (32, 1),
                _ => return None,
            })
        }
        Type::Array(arr) => {
            let (elem_size, elem_align) = size_align(&arr.elem)?;
            let n = array_len(&arr.len)?;
            Some((elem_size * n, elem_align))
        }
        _ => None,
    }
}

fn array_len(expr: &syn::Expr) -> Option<usize> {
    if let syn::Expr::Lit(syn::ExprLit {
        lit: syn::Lit::Int(i),
        ..
    }) = expr
    {
        i.base10_parse::<usize>().ok()
    } else {
        None
    }
}
