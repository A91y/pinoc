//! Per-account fact table. For each instruction handler it records how each
//! account binding was validated and used, so account/CPI lints reduce to short
//! predicates over the table. syn-only: when an account flows through a construct
//! it can't follow (passed to a user function, moved into a struct), the binding
//! is marked `delegated` and lints stay quiet rather than guess.

#![allow(dead_code)]

use std::collections::{HashMap, HashSet};
use syn::spanned::Spanned;
use syn::visit::Visit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    SlicePattern,
    Index,
    Iter,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Validation {
    Signer,
    Owner,
    Key,
    Writable,
    Uninitialized,
    Discriminator,
    LengthChecked,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Use {
    BorrowData { mut_: bool, unchecked: bool },
    DeserializeState(String),
    LamportsRead,
    LamportsMut,
    CpiAccount,
    InvokeSignedSeeds,
    AssignOwner,
    CloseDrainLamports,
}

pub struct AccountBinding {
    pub name: String,
    pub origin: Origin,
    pub validations: HashSet<Validation>,
    pub uses: HashSet<Use>,
    pub delegated: bool,
    /// Span of the first use that reads the account's data, for finding location.
    pub read_span: Option<proc_macro2::Span>,
}

impl AccountBinding {
    fn new(name: String, origin: Origin) -> Self {
        Self {
            name,
            origin,
            validations: HashSet::new(),
            uses: HashSet::new(),
            delegated: false,
            read_span: None,
        }
    }

    pub fn reads_data(&self) -> bool {
        self.uses
            .iter()
            .any(|u| matches!(u, Use::BorrowData { .. } | Use::DeserializeState(_)))
    }

    /// The account's data is borrowed through an `*_unchecked` accessor, which
    /// skips the safe path's bounds handling.
    pub fn unchecked_read(&self) -> bool {
        self.uses.iter().any(|u| {
            matches!(
                u,
                Use::BorrowData {
                    unchecked: true,
                    ..
                }
            )
        })
    }
}

pub struct Handler {
    pub name: String,
    pub bindings: Vec<AccountBinding>,
}

const ACCOUNT_SLICE_TYPES: &[&str] = &["AccountInfo", "AccountView"];
const LOADER_METHODS: &[&str] = &[
    "load",
    "load_mut",
    "from_bytes",
    "from_account_info",
    "from_account_view",
    "try_from_slice",
    "unpack",
    "deserialize",
];
const SYSVAR_TYPES: &[&str] = &[
    "Rent",
    "Clock",
    "EpochSchedule",
    "Fees",
    "SlotHashes",
    "StakeHistory",
    "Instructions",
    "RecentBlockhashes",
    "EpochRewards",
    "LastRestartSlot",
];
const CPI_FUNCS: &[&str] = &["invoke", "invoke_signed"];

fn validation_for_method(name: &str) -> Option<Validation> {
    Some(match name {
        "is_signer" => Validation::Signer,
        "owner" => Validation::Owner,
        "key" | "address" => Validation::Key,
        "is_writable" => Validation::Writable,
        "is_data_empty" | "data_is_empty" => Validation::Uninitialized,
        "data_len" => Validation::LengthChecked,
        _ => return None,
    })
}

fn borrow_use_for_method(name: &str) -> Option<Use> {
    Some(match name {
        "try_borrow_data" | "try_borrow" => Use::BorrowData {
            mut_: false,
            unchecked: false,
        },
        "try_borrow_mut_data" | "try_borrow_mut" => Use::BorrowData {
            mut_: true,
            unchecked: false,
        },
        "borrow_data_unchecked" => Use::BorrowData {
            mut_: false,
            unchecked: true,
        },
        "borrow_mut_data_unchecked" => Use::BorrowData {
            mut_: true,
            unchecked: true,
        },
        _ => return None,
    })
}

/// Extracts a fact table for every instruction handler in `file`. A handler is a
/// `fn` taking a `&[AccountInfo]`/`&[AccountView]` slice.
pub fn extract_handlers(file: &syn::File) -> Vec<Handler> {
    let mut handlers = Vec::new();
    collect_from_items(&file.items, &mut handlers);
    handlers
}

fn collect_from_items(items: &[syn::Item], out: &mut Vec<Handler>) {
    for item in items {
        match item {
            syn::Item::Fn(f) => {
                if let Some(accounts_param) = accounts_param_name(&f.sig) {
                    out.push(extract_handler(
                        &f.sig.ident.to_string(),
                        accounts_param,
                        &f.block,
                    ));
                }
            }
            syn::Item::Mod(m) => {
                if let Some((_, inner)) = &m.content {
                    collect_from_items(inner, out);
                }
            }
            _ => {}
        }
    }
}

/// The parameter name of the accounts slice, if the signature has one.
fn accounts_param_name(sig: &syn::Signature) -> Option<String> {
    for input in &sig.inputs {
        let syn::FnArg::Typed(pat_ty) = input else {
            continue;
        };
        if !is_account_slice(&pat_ty.ty) {
            continue;
        }
        if let syn::Pat::Ident(p) = pat_ty.pat.as_ref() {
            return Some(p.ident.to_string());
        }
    }
    None
}

fn is_account_slice(ty: &syn::Type) -> bool {
    let syn::Type::Reference(r) = ty else {
        return false;
    };
    let syn::Type::Slice(s) = r.elem.as_ref() else {
        return false;
    };
    last_segment_ident(&s.elem).is_some_and(|id| ACCOUNT_SLICE_TYPES.contains(&id.as_str()))
}

fn extract_handler(name: &str, accounts_param: String, block: &syn::Block) -> Handler {
    let mut ex = Extractor {
        accounts_param,
        bindings: Vec::new(),
        index: HashMap::new(),
        aliases: HashMap::new(),
        macro_texts: Vec::new(),
    };
    ex.visit_block(block);
    ex.apply_macro_validations();
    Handler {
        name: name.to_string(),
        bindings: ex.bindings,
    }
}

struct Extractor {
    accounts_param: String,
    bindings: Vec<AccountBinding>,
    index: HashMap<String, usize>,
    aliases: HashMap<String, String>,
    macro_texts: Vec<String>,
}

impl Extractor {
    fn add_binding(&mut self, name: String, origin: Origin) {
        if self.index.contains_key(&name) {
            return;
        }
        self.index.insert(name.clone(), self.bindings.len());
        self.bindings.push(AccountBinding::new(name, origin));
    }

    fn binding_idx(&self, name: &str) -> Option<usize> {
        let canonical = self.aliases.get(name).map(String::as_str).unwrap_or(name);
        self.index.get(canonical).copied()
    }

    /// Peels references/parens/deref and returns the binding index of a bare name.
    fn resolve(&self, expr: &syn::Expr) -> Option<usize> {
        match expr {
            syn::Expr::Reference(r) => self.resolve(&r.expr),
            syn::Expr::Paren(p) => self.resolve(&p.expr),
            syn::Expr::Group(g) => self.resolve(&g.expr),
            syn::Expr::Unary(syn::ExprUnary {
                op: syn::UnOp::Deref(_),
                expr,
                ..
            }) => self.resolve(expr),
            syn::Expr::Path(p) => {
                let name = p.path.get_ident()?.to_string();
                self.binding_idx(&name)
            }
            _ => None,
        }
    }

    fn apply_macro_validations(&mut self) {
        // Owner/signer/key checks are often written inside macros (`assert_eq!`,
        // `require!`), whose bodies syn does not visit as expressions. Scan the
        // macro token text for `<binding> . method` and record the validation.
        let texts = std::mem::take(&mut self.macro_texts);
        for i in 0..self.bindings.len() {
            let name = self.bindings[i].name.clone();
            for text in &texts {
                for (method, val) in [
                    ("owner", Validation::Owner),
                    ("key", Validation::Key),
                    ("address", Validation::Key),
                    ("is_signer", Validation::Signer),
                    ("is_writable", Validation::Writable),
                    ("is_data_empty", Validation::Uninitialized),
                    ("data_is_empty", Validation::Uninitialized),
                    ("data_len", Validation::LengthChecked),
                ] {
                    if text.contains(&format!("{name} . {method}")) {
                        self.bindings[i].validations.insert(val.clone());
                    }
                }
            }
        }
    }
}

impl<'ast> Visit<'ast> for Extractor {
    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let Some(init) = &local.init {
            self.discover_binding(&local.pat, &init.expr);
        }
        syn::visit::visit_local(self, local);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if let Some(idx) = self.resolve(&node.receiver) {
            let method = node.method.to_string();
            if let Some(v) = validation_for_method(&method) {
                self.bindings[idx].validations.insert(v);
            } else if let Some(u) = borrow_use_for_method(&method) {
                if self.bindings[idx].read_span.is_none() {
                    self.bindings[idx].read_span = Some(node.method.span());
                }
                self.bindings[idx].uses.insert(u);
            } else if method == "lamports" {
                self.bindings[idx].uses.insert(Use::LamportsRead);
            } else if method == "assign" {
                self.bindings[idx].uses.insert(Use::AssignOwner);
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        self.classify_call(node);
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.macro_texts.push(mac.tokens.to_string());
        syn::visit::visit_macro(self, mac);
    }
}

impl Extractor {
    fn discover_binding(&mut self, pat: &syn::Pat, init: &syn::Expr) {
        match pat {
            // `let [a, b, ..] = accounts else { ... }`
            syn::Pat::Slice(slice) if self.is_accounts_expr(init) => {
                for elem in &slice.elems {
                    if let syn::Pat::Ident(p) = elem {
                        self.add_binding(p.ident.to_string(), Origin::SlicePattern);
                    }
                }
            }
            syn::Pat::Ident(p) => {
                let name = p.ident.to_string();
                if self.is_index_of_accounts(init) {
                    self.add_binding(name, Origin::Index);
                } else if self.is_iter_next(init) {
                    self.add_binding(name, Origin::Iter);
                } else if let Some(src) = self.resolve(init) {
                    // `let y = x;` renames binding x.
                    let canonical = self.bindings[src].name.clone();
                    self.aliases.insert(name, canonical);
                }
            }
            _ => {}
        }
    }

    fn is_accounts_expr(&self, expr: &syn::Expr) -> bool {
        matches!(expr, syn::Expr::Path(p) if p.path.is_ident(self.accounts_param.as_str()))
    }

    fn is_index_of_accounts(&self, expr: &syn::Expr) -> bool {
        let expr = strip_ref(expr);
        match expr {
            syn::Expr::Index(idx) => self.is_accounts_expr(&idx.expr),
            syn::Expr::MethodCall(m) => m.method == "get" && self.is_accounts_expr(&m.receiver),
            _ => false,
        }
    }

    fn is_iter_next(&self, expr: &syn::Expr) -> bool {
        let expr = strip_try(expr);
        matches!(expr, syn::Expr::Call(c)
            if last_call_segment(&c.func).is_some_and(|s| s == "next_account_info"))
    }

    fn classify_call(&mut self, node: &syn::ExprCall) {
        let Some((type_seg, fn_seg)) = call_path_tail(&node.func) else {
            // Not a path call; nothing to classify.
            return;
        };

        // `invoke`/`invoke_signed(...)`: account args are CPI accounts.
        if type_seg.is_none() && CPI_FUNCS.contains(&fn_seg.as_str()) {
            for arg in &node.args {
                if let Some(idx) = self.resolve(arg) {
                    self.bindings[idx].uses.insert(Use::CpiAccount);
                }
            }
            return;
        }

        // `Type::loader(account)`: reading the account as program state.
        if let Some(ty) = &type_seg {
            if LOADER_METHODS.contains(&fn_seg.as_str()) && !SYSVAR_TYPES.contains(&ty.as_str()) {
                for arg in &node.args {
                    if let Some(idx) = self.resolve(arg) {
                        if self.bindings[idx].read_span.is_none() {
                            self.bindings[idx].read_span = Some(node.span());
                        }
                        self.bindings[idx]
                            .uses
                            .insert(Use::DeserializeState(ty.clone()));
                    }
                }
                return;
            }
        }

        // Any other call taking a bare account arg is opaque: mark it delegated.
        for arg in &node.args {
            if let Some(idx) = self.resolve(arg) {
                self.bindings[idx].delegated = true;
            }
        }
    }
}

fn strip_ref(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::Reference(r) => strip_ref(&r.expr),
        syn::Expr::Paren(p) => strip_ref(&p.expr),
        _ => expr,
    }
}

fn strip_try(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::Try(t) => strip_try(&t.expr),
        syn::Expr::Paren(p) => strip_try(&p.expr),
        _ => expr,
    }
}

fn last_segment_ident(ty: &syn::Type) -> Option<String> {
    if let syn::Type::Path(p) = ty {
        return p.path.segments.last().map(|s| s.ident.to_string());
    }
    None
}

fn last_call_segment(func: &syn::Expr) -> Option<String> {
    if let syn::Expr::Path(p) = func {
        return p.path.segments.last().map(|s| s.ident.to_string());
    }
    None
}

/// For `a::b::C::method` returns `(Some("C"), "method")`; for `func` returns
/// `(None, "func")`.
fn call_path_tail(func: &syn::Expr) -> Option<(Option<String>, String)> {
    let syn::Expr::Path(p) = func else {
        return None;
    };
    let segs = &p.path.segments;
    let fn_seg = segs.last()?.ident.to_string();
    let type_seg = if segs.len() >= 2 {
        Some(segs[segs.len() - 2].ident.to_string())
    } else {
        None
    };
    Some((type_seg, fn_seg))
}
