//! Fallback used when `shank_idl` finds no errors, since it only recognizes
//! enums deriving `thiserror::Error`: scans `src/` for an enum manually
//! `impl From<X> for ProgramError`'d instead, synthesizing messages from
//! variant names (`NotDone` -> "Not Done"). Codes are the values the `From`
//! impl passes to `ProgramError::Custom`, so an offset such as
//! `e as u32 + 6000` is included.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use syn::{
    BinOp, Expr, ExprLit, FnArg, GenericArgument, ImplItem, ImplItemFn, Item, ItemEnum, ItemImpl,
    Lit, Pat, PathArguments, Stmt, Type,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualErrorCode {
    pub code: u32,
    pub name: String,
    pub msg: Option<String>,
}

/// The error enum behind a manual `impl From<X> for ProgramError`.
pub struct ManualErrors {
    pub errors: Vec<ManualErrorCode>,
    /// `(variant, raw discriminant)`, in declaration order.
    pub discriminants: Vec<(String, u32)>,
    /// How the `From` impl turns a variant into its `ProgramError::Custom` code.
    pub conversion: Conversion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conversion {
    /// Discriminant plus a constant; `0` for a plain `e as u32`.
    Offset(i64),
    /// A `match` giving each variant its own literal code.
    PerVariant,
    /// Not a form this module can evaluate; codes are the raw discriminants.
    Unknown,
}

/// Scans `src_dir` for a manually-implemented `ProgramError` source enum and
/// returns its variants with the codes the program returns for them, or `None`
/// if no such enum is found.
pub fn find_manual_program_errors(src_dir: &Path) -> Result<Option<ManualErrors>> {
    let mut items = CrateItems::default();
    collect_items(src_dir, &mut items)?;

    let Some((enum_item, from_impl)) = items.enums.iter().find_map(|e| {
        let from = items
            .from_impls
            .iter()
            .find(|(target, _)| e.ident == target)?;
        Some((e, &from.1))
    }) else {
        return Ok(None);
    };

    let enum_name = enum_item.ident.to_string();
    let mut discriminants = Vec::new();
    let mut next: i64 = 0;
    for variant in &enum_item.variants {
        if let Some((_, expr)) = &variant.discriminant {
            if let Some(value) = items.eval_const(expr, &enum_name, 0) {
                next = value;
            }
        }
        discriminants.push((variant.ident.to_string(), next));
        next += 1;
    }

    let rule = items.conversion_rule(&enum_name, from_impl);
    let conversion = match &rule {
        Rule::Offset(k) => Conversion::Offset(*k),
        Rule::PerVariant(map) if discriminants.iter().all(|(name, _)| map.contains_key(name)) => {
            Conversion::PerVariant
        }
        _ => Conversion::Unknown,
    };

    let errors = discriminants
        .iter()
        .map(|(name, discriminant)| {
            let code = match (&conversion, &rule) {
                (Conversion::Offset(k), _) => discriminant + k,
                (Conversion::PerVariant, Rule::PerVariant(map)) => map[name],
                _ => *discriminant,
            };
            ManualErrorCode {
                code: u32::try_from(code).unwrap_or_default(),
                msg: Some(pascal_case_to_words(name)),
                name: name.clone(),
            }
        })
        .collect();
    let discriminants = discriminants
        .into_iter()
        .map(|(name, d)| (name, u32::try_from(d).unwrap_or_default()))
        .collect();

    Ok(Some(ManualErrors {
        errors,
        discriminants,
        conversion,
    }))
}

/// Replaces the codes of `errors` (shank's thiserror-derived list) with the ones
/// `manual` computed. `Ok(None)` leaves shank's list as it is because nothing
/// needs converting; `Err` names the variant that shows the `From` impl is for
/// a different enum, so its conversion could not be applied.
pub fn with_converted_codes(
    mut errors: Vec<ManualErrorCode>,
    manual: &ManualErrors,
) -> std::result::Result<Option<Vec<ManualErrorCode>>, String> {
    let codes: HashMap<&str, u32> = manual
        .errors
        .iter()
        .map(|e| (e.name.as_str(), e.code))
        .collect();
    let mut changed = false;
    for error in &mut errors {
        let Some(code) = codes.get(error.name.as_str()) else {
            return Err(error.name.clone());
        };
        changed |= error.code != *code;
        error.code = *code;
    }
    Ok(changed.then_some(errors))
}

impl ManualErrors {
    /// True when the `From` impl does more than cast the discriminant, in a way
    /// this module evaluated.
    pub fn changes_codes(&self) -> bool {
        !matches!(self.conversion, Conversion::Offset(0) | Conversion::Unknown)
    }
}

/// The value of a code expression, as a function of the enum variant.
enum Rule {
    Const(i64),
    /// Discriminant plus a constant.
    Offset(i64),
    PerVariant(HashMap<String, i64>),
    Unknown,
}

/// Everything in the crate the conversion can refer to.
#[derive(Default)]
struct CrateItems {
    enums: Vec<ItemEnum>,
    /// `(X, impl From<X> for ProgramError)`.
    from_impls: Vec<(String, ItemImpl)>,
    /// Free `const NAME: _ = <expr>`.
    consts: HashMap<String, Expr>,
    /// `impl Type { const NAME: _ = <expr> }`, keyed by `(Type, NAME)`.
    assoc_consts: HashMap<(String, String), Expr>,
    /// `impl Type { fn method(self) .. }`, keyed by `(Type, method)`.
    methods: HashMap<(String, String), ImplItemFn>,
}

const MAX_DEPTH: usize = 8;

impl CrateItems {
    fn conversion_rule(&self, enum_name: &str, from_impl: &ItemImpl) -> Rule {
        let Some(from_fn) = from_impl.items.iter().find_map(|item| match item {
            ImplItem::Fn(f) if f.sig.ident == "from" => Some(f),
            _ => None,
        }) else {
            return Rule::Unknown;
        };
        let Some(param) = first_param_name(&from_fn.sig) else {
            return Rule::Unknown;
        };
        let Some(body) = block_value(&from_fn.block) else {
            return Rule::Unknown;
        };
        self.program_error_code(body, enum_name, &param, 0)
    }

    /// Evaluates an expression of type `ProgramError`: `ProgramError::Custom(<code>)`,
    /// or a method on the enum that returns one.
    fn program_error_code(&self, expr: &Expr, enum_name: &str, param: &str, depth: usize) -> Rule {
        if depth > MAX_DEPTH {
            return Rule::Unknown;
        }
        match peel(expr) {
            Expr::Call(call) if path_ends_with(&call.func, "Custom") && call.args.len() == 1 => {
                self.code(&call.args[0], enum_name, param, depth + 1)
            }
            Expr::MethodCall(m) if is_ident(&m.receiver, param) && m.args.is_empty() => {
                match self.method_body(enum_name, &m.method.to_string()) {
                    Some(body) => self.program_error_code(body, enum_name, "self", depth + 1),
                    None => Rule::Unknown,
                }
            }
            _ => Rule::Unknown,
        }
    }

    /// Evaluates a code expression, where `param` names the enum value.
    fn code(&self, expr: &Expr, enum_name: &str, param: &str, depth: usize) -> Rule {
        if depth > MAX_DEPTH {
            return Rule::Unknown;
        }
        let expr = peel(expr);
        if let Some(value) = self.eval_const(expr, enum_name, depth) {
            return Rule::Const(value);
        }
        match expr {
            Expr::Path(_) if is_ident(expr, param) => Rule::Offset(0),
            Expr::Cast(cast) => self.code(&cast.expr, enum_name, param, depth + 1),
            Expr::Binary(bin) => {
                let left = self.code(&bin.left, enum_name, param, depth + 1);
                let right = self.code(&bin.right, enum_name, param, depth + 1);
                match (&bin.op, left, right) {
                    (BinOp::Add(_), Rule::Offset(a), Rule::Const(b))
                    | (BinOp::Add(_), Rule::Const(b), Rule::Offset(a)) => Rule::Offset(a + b),
                    (BinOp::Sub(_), Rule::Offset(a), Rule::Const(b)) => Rule::Offset(a - b),
                    _ => Rule::Unknown,
                }
            }
            Expr::MethodCall(m) if is_ident(&m.receiver, param) && m.args.is_empty() => {
                match self.method_body(enum_name, &m.method.to_string()) {
                    Some(body) => self.code(body, enum_name, "self", depth + 1),
                    None => Rule::Unknown,
                }
            }
            Expr::Match(m) if is_ident(&m.expr, param) => {
                let mut map = HashMap::new();
                for arm in &m.arms {
                    let (Some(variant), None) = (variant_of(&arm.pat, enum_name), &arm.guard)
                    else {
                        return Rule::Unknown;
                    };
                    let Some(value) = self.eval_const(peel(&arm.body), enum_name, depth + 1) else {
                        return Rule::Unknown;
                    };
                    map.insert(variant, value);
                }
                Rule::PerVariant(map)
            }
            _ => Rule::Unknown,
        }
    }

    /// Evaluates an integer constant expression: literals, named consts, and
    /// `+`/`-`/`*` over them.
    fn eval_const(&self, expr: &Expr, self_ty: &str, depth: usize) -> Option<i64> {
        if depth > MAX_DEPTH {
            return None;
        }
        match peel(expr) {
            Expr::Lit(ExprLit {
                lit: Lit::Int(int), ..
            }) => int.base10_parse::<i64>().ok(),
            Expr::Cast(cast) => self.eval_const(&cast.expr, self_ty, depth + 1),
            Expr::Binary(bin) => {
                let left = self.eval_const(&bin.left, self_ty, depth + 1)?;
                let right = self.eval_const(&bin.right, self_ty, depth + 1)?;
                match bin.op {
                    BinOp::Add(_) => left.checked_add(right),
                    BinOp::Sub(_) => left.checked_sub(right),
                    BinOp::Mul(_) => left.checked_mul(right),
                    _ => None,
                }
            }
            Expr::Path(path) => {
                let segments = &path.path.segments;
                let name = segments.last()?.ident.to_string();
                if segments.len() >= 2 {
                    let owner = segments[segments.len() - 2].ident.to_string();
                    let owner = if owner == "Self" { self_ty } else { &owner };
                    if let Some(value) = self.assoc_consts.get(&(owner.to_string(), name.clone())) {
                        return self.eval_const(value, owner, depth + 1);
                    }
                }
                self.eval_const(self.consts.get(&name)?, self_ty, depth + 1)
            }
            _ => None,
        }
    }

    fn method_body(&self, ty: &str, method: &str) -> Option<&Expr> {
        block_value(
            &self
                .methods
                .get(&(ty.to_string(), method.to_string()))?
                .block,
        )
    }
}

/// The value of a block that is a single expression, optionally `return`ed.
fn block_value(block: &syn::Block) -> Option<&Expr> {
    let [stmt] = block.stmts.as_slice() else {
        return None;
    };
    match stmt {
        Stmt::Expr(Expr::Return(ret), _) => ret.expr.as_deref(),
        Stmt::Expr(expr, _) => Some(expr),
        _ => None,
    }
}

/// Strips parentheses, groups, single-expression blocks, and `.into()`.
fn peel(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(e) => peel(&e.expr),
        Expr::Group(e) => peel(&e.expr),
        Expr::Block(b) => block_value(&b.block).map(peel).unwrap_or(expr),
        Expr::MethodCall(m) if m.method == "into" && m.args.is_empty() => peel(&m.receiver),
        _ => expr,
    }
}

fn is_ident(expr: &Expr, name: &str) -> bool {
    match peel(expr) {
        Expr::Path(p) => p.path.is_ident(name),
        Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Deref(_),
            expr,
            ..
        }) => is_ident(expr, name),
        _ => false,
    }
}

fn path_ends_with(expr: &Expr, name: &str) -> bool {
    matches!(expr, Expr::Path(p) if p.path.segments.last().is_some_and(|s| s.ident == name))
}

fn first_param_name(sig: &syn::Signature) -> Option<String> {
    match sig.inputs.first()? {
        FnArg::Typed(pat_ty) => match pat_ty.pat.as_ref() {
            Pat::Ident(p) => Some(p.ident.to_string()),
            _ => None,
        },
        FnArg::Receiver(_) => Some("self".to_string()),
    }
}

/// The variant name of a unit-variant pattern `Enum::Variant` / `Self::Variant`.
fn variant_of(pat: &Pat, enum_name: &str) -> Option<String> {
    let Pat::Path(p) = pat else {
        return None;
    };
    let segments = &p.path.segments;
    if segments.len() < 2 {
        return None;
    }
    let owner = &segments[segments.len() - 2].ident;
    (owner == enum_name || owner == "Self").then(|| segments.last().unwrap().ident.to_string())
}

fn collect_items(dir: &Path, out: &mut CrateItems) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_items(&path, out)?;
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
        collect_from(file.items, out);
    }
    Ok(())
}

fn collect_from(items: Vec<Item>, out: &mut CrateItems) {
    for item in items {
        match item {
            Item::Enum(item_enum) => out.enums.push(item_enum),
            Item::Const(c) => {
                out.consts.insert(c.ident.to_string(), *c.expr);
            }
            Item::Mod(m) => {
                if let Some((_, inner)) = m.content {
                    collect_from(inner, out);
                }
            }
            Item::Impl(item_impl) => {
                if let Some(target) = manual_program_error_target(&item_impl) {
                    out.from_impls.push((target, item_impl));
                } else if item_impl.trait_.is_none() {
                    let Type::Path(ty) = item_impl.self_ty.as_ref() else {
                        continue;
                    };
                    let Some(ty) = ty.path.segments.last().map(|s| s.ident.to_string()) else {
                        continue;
                    };
                    for item in item_impl.items {
                        match item {
                            ImplItem::Const(c) => {
                                out.assoc_consts
                                    .insert((ty.clone(), c.ident.to_string()), c.expr);
                            }
                            ImplItem::Fn(f) => {
                                out.methods.insert((ty.clone(), f.sig.ident.to_string()), f);
                            }
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
    }
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
