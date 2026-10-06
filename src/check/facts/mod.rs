//! Per-account fact table. For each instruction handler it records how each
//! account binding was validated and used, so account/CPI lints reduce to short
//! predicates over the table. syn-only, built over the whole crate:
//!
//! - A call to a function defined in the crate applies that function's
//!   **summary**: what its body checks and uses on each account it is given,
//!   following nested calls. A call to anything else marks the account
//!   `delegated`, and lints stay quiet rather than guess.
//! - Accounts a method moves into its own type (`Self { vault, .. }`) are
//!   followed into every function that reaches them through that type
//!   (`self.vault`, `self.accounts.vault`, `ix.accounts.vault`).

#![allow(dead_code)]

use crate::check::contract::{ParsedFile, Span};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use syn::spanned::Spanned;
use syn::visit::Visit;

/// A source location in one of the crate's files.
#[derive(Clone)]
pub struct Site {
    pub file: String,
    pub span: proc_macro2::Span,
}

impl Site {
    /// Line is 1-indexed, column 0-indexed.
    pub fn to_span(&self) -> Span {
        let start = self.span.start();
        Span {
            file: self.file.clone(),
            line: start.line,
            col: start.column,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    SlicePattern,
    Index,
    Iter,
    /// An account parameter of a helper function, while its summary is built.
    Param,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Validation {
    Signer,
    Owner,
    Key,
    /// Key compared (`==`/`!=` or in a macro), not merely read like `Key`.
    KeyCompared,
    Writable,
    Uninitialized,
    Discriminator,
    LengthChecked,
    /// The handler creates the account itself (`CreateAccount { to: account, .. }`),
    /// so its owner and contents are the program's own.
    Created,
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
    /// The first use that reads the account's data, for finding location.
    pub read_site: Option<Site>,
    /// The function that read happened in.
    pub read_in: Option<String>,
    /// The comparison of this key against an authority-named field/var (the
    /// account acts as an authority); `None` if there is no such comparison.
    pub authority_site: Option<Site>,
}

impl AccountBinding {
    /// The account cannot be a look-alike supplied by the caller: its owner was
    /// checked, its address was compared to an expected one, or the handler
    /// found it empty or created it, so the data it reads is its own.
    pub fn identity_established(&self) -> bool {
        [
            Validation::Owner,
            Validation::KeyCompared,
            Validation::Uninitialized,
            Validation::Created,
        ]
        .iter()
        .any(|v| self.validations.contains(v))
    }

    fn new(name: String, origin: Origin) -> Self {
        Self {
            name,
            origin,
            validations: HashSet::new(),
            uses: HashSet::new(),
            delegated: false,
            read_site: None,
            read_in: None,
            authority_site: None,
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
    /// `function` for a free function, `Type::method` for a method.
    pub name: String,
    pub file: String,
    pub bindings: Vec<AccountBinding>,
    pub cpi_sites: Vec<CpiSite>,
    /// Where the handler moves account bindings into its own type (`Self { vault, .. }`).
    pub stores_accounts: Option<Site>,
    /// The functions that reach the stored accounts through that type and were
    /// analysed as part of this handler.
    pub followed_by: Vec<String>,
}

impl Handler {
    /// For evidence text: where a use happened, when not in the handler itself.
    pub fn used_in(&self, binding: &AccountBinding) -> String {
        match &binding.read_in {
            Some(function) if *function != self.name => {
                format!(" (in `{function}`, bound in `{}`)", self.name)
            }
            _ => String::new(),
        }
    }
}

/// One `invoke`/`invoke_signed` call site.
#[derive(Clone)]
pub struct CpiSite {
    /// Binding whose key feeds the program id; `None` for a constant/param id.
    pub program_binding: Option<usize>,
    pub site: Site,
}

const ACCOUNT_SLICE_TYPES: &[&str] = &["AccountInfo", "AccountView"];
const ACCOUNT_ITER_FUNCS: &[&str] = &["next_account_info", "next_account_view"];
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
/// System-program builders that create the account in their `to` field.
const CREATE_BUILDERS: &[&str] = &["CreateAccount", "CreateAccountWithSeed"];
const INSTRUCTION_TYPES: &[&str] = &["Instruction", "InstructionView"];
/// Macros that only format or log; a field mention inside them is not a check.
const LOG_MACROS: &[&str] = &[
    "msg", "log", "sol_log", "println", "print", "eprintln", "eprint", "format", "write",
    "writeln", "panic", "dbg", "emit",
];

fn validation_for_method(name: &str) -> Option<Validation> {
    Some(match name {
        "is_signer" => Validation::Signer,
        "owner" | "owned_by" | "is_owned_by" => Validation::Owner,
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
        "borrow_data_unchecked" | "borrow_unchecked" => Use::BorrowData {
            mut_: false,
            unchecked: true,
        },
        "borrow_mut_data_unchecked" | "borrow_unchecked_mut" => Use::BorrowData {
            mut_: true,
            unchecked: true,
        },
        _ => return None,
    })
}

/// A function or method defined in the crate.
struct FnInfo<'a> {
    /// The `impl` type or trait it belongs to, if any.
    owner: Option<String>,
    /// A default method of a trait, callable as `AnyImplementor::name`.
    trait_default: bool,
    /// Module path below `src/`, e.g. `["token", "account"]`.
    module: Vec<String>,
    name: String,
    sig: &'a syn::Signature,
    block: &'a syn::Block,
    file: String,
}

impl FnInfo<'_> {
    fn display(&self) -> String {
        match &self.owner {
            Some(owner) => format!("{owner}::{}", self.name),
            None => self.name.clone(),
        }
    }

    fn has_receiver(&self) -> bool {
        matches!(self.sig.inputs.first(), Some(syn::FnArg::Receiver(_)))
    }

    /// `(name, kind, type name)` of each typed parameter, in order.
    fn params(&self) -> Vec<(Option<String>, ParamKind, Option<String>)> {
        self.sig
            .inputs
            .iter()
            .filter_map(|input| match input {
                syn::FnArg::Typed(pat_ty) => {
                    let name = match pat_ty.pat.as_ref() {
                        syn::Pat::Ident(p) => Some(p.ident.to_string()),
                        _ => None,
                    };
                    let ty = peel_type(&pat_ty.ty);
                    Some((name, param_kind(&pat_ty.ty), last_segment_ident(ty)))
                }
                syn::FnArg::Receiver(_) => None,
            })
            .collect()
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ParamKind {
    Account,
    Bytes,
    Other,
}

fn peel_type(ty: &syn::Type) -> &syn::Type {
    match ty {
        syn::Type::Reference(r) => peel_type(&r.elem),
        syn::Type::Paren(p) => peel_type(&p.elem),
        _ => ty,
    }
}

fn param_kind(ty: &syn::Type) -> ParamKind {
    match peel_type(ty) {
        syn::Type::Slice(s) => match s.elem.as_ref() {
            syn::Type::Path(p) if p.path.is_ident("u8") => ParamKind::Bytes,
            _ => ParamKind::Other,
        },
        other => match last_segment_ident(other) {
            Some(id) if ACCOUNT_SLICE_TYPES.contains(&id.as_str()) => ParamKind::Account,
            _ => ParamKind::Other,
        },
    }
}

/// What a function does with each of its parameters.
struct Summary {
    params: Vec<ParamSummary>,
}

enum ParamSummary {
    Account {
        validations: HashSet<Validation>,
        uses: HashSet<Use>,
        delegated: bool,
        /// Its key is compared against an authority-named value.
        authority: bool,
        /// Its key is the program id of a raw `invoke`.
        invoked_as_program: bool,
    },
    /// A byte slice; `len_checked` when its length is tested before use.
    Bytes {
        len_checked: bool,
    },
    Other,
}

/// Every function and struct in the crate, and the summaries built from them.
struct Analyzer<'a> {
    fns: Vec<FnInfo<'a>>,
    by_name: HashMap<String, Vec<usize>>,
    /// Struct name to `(field, field type name)`.
    structs: HashMap<String, Vec<(String, String)>>,
    summaries: RefCell<HashMap<usize, Option<Rc<Summary>>>>,
}

impl<'a> Analyzer<'a> {
    fn new(files: &'a [ParsedFile]) -> Self {
        let mut analyzer = Analyzer {
            fns: Vec::new(),
            by_name: HashMap::new(),
            structs: HashMap::new(),
            summaries: RefCell::new(HashMap::new()),
        };
        for file in files {
            let path = file.path.display().to_string();
            let module = module_path(&file.path);
            analyzer.index_items(&file.ast.items, &path, &module);
        }
        for (index, info) in analyzer.fns.iter().enumerate() {
            analyzer
                .by_name
                .entry(info.name.clone())
                .or_default()
                .push(index);
        }
        analyzer
    }

    fn index_items(&mut self, items: &'a [syn::Item], file: &str, module: &[String]) {
        let mut push = |owner: Option<String>, trait_default: bool, sig, block| {
            self.fns.push(FnInfo {
                owner,
                trait_default,
                module: module.to_vec(),
                name: String::new(),
                sig,
                block,
                file: file.to_string(),
            });
        };
        let mut nested = Vec::new();
        for item in items {
            match item {
                syn::Item::Fn(f) => push(None, false, &f.sig, &f.block),
                syn::Item::Impl(imp) => {
                    let ty = last_segment_ident(&imp.self_ty);
                    for item in &imp.items {
                        if let syn::ImplItem::Fn(f) = item {
                            push(ty.clone(), false, &f.sig, &f.block);
                        }
                    }
                }
                syn::Item::Trait(t) => {
                    for item in &t.items {
                        if let syn::TraitItem::Fn(f) = item {
                            if let Some(block) = &f.default {
                                push(Some(t.ident.to_string()), true, &f.sig, block);
                            }
                        }
                    }
                }
                syn::Item::Struct(st) => {
                    let fields = st
                        .fields
                        .iter()
                        .filter_map(|f| {
                            let ty = last_segment_ident(peel_type(&f.ty))?;
                            Some((f.ident.as_ref()?.to_string(), ty))
                        })
                        .collect();
                    // Two structs may share a name across modules; keep both sets
                    // of fields, since a path alone does not say which is meant.
                    self.structs
                        .entry(st.ident.to_string())
                        .or_default()
                        .extend::<Vec<(String, String)>>(fields);
                }
                syn::Item::Mod(m) => {
                    if let Some((_, inner)) = &m.content {
                        nested.push((m.ident.to_string(), inner));
                    }
                }
                _ => {}
            }
        }
        for info in &mut self.fns {
            if info.name.is_empty() {
                info.name = info.sig.ident.to_string();
            }
        }
        for (name, inner) in nested {
            let mut module = module.to_vec();
            module.push(name);
            self.index_items(inner, file, &module);
        }
    }

    /// The crate function a call path names, if exactly one fits. `owner` is
    /// the type `Self` stands for at the call site.
    fn resolve_callee(&self, segments: &[String], owner: Option<&str>) -> Option<usize> {
        let (name, qualifier) = match segments {
            [.., qualifier, name] => (name, Some(qualifier.as_str())),
            [name] => (name, None),
            [] => return None,
        };
        let candidates = self.by_name.get(name)?;
        let fits = |info: &FnInfo| match qualifier {
            // A bare call names a free function.
            None => info.owner.is_none(),
            Some("crate" | "super" | "self") => info.owner.is_none(),
            Some("Self") => info.owner.as_deref() == owner && owner.is_some(),
            Some(q) => {
                info.owner.as_deref() == Some(q)
                    || (info.owner.is_none() && info.module.last().map(String::as_str) == Some(q))
            }
        };
        let unique = |keep: &dyn Fn(&FnInfo) -> bool| {
            let mut matching = candidates.iter().copied().filter(|i| keep(&self.fns[*i]));
            match (matching.next(), matching.next()) {
                (Some(only), None) => Some(only),
                _ => None,
            }
        };
        // `Config::from_bytes` may be a default method of a trait `Config`
        // implements, when the type has no method of that name itself.
        unique(&fits).or_else(|| {
            let on_a_type = qualifier.is_some_and(|q| !matches!(q, "crate" | "super" | "self"));
            let any_fit = candidates.iter().any(|i| fits(&self.fns[*i]));
            (on_a_type && !any_fit)
                .then(|| unique(&|info: &FnInfo| info.trait_default))
                .flatten()
        })
    }

    /// The summary of a crate function. `None` while it is being computed, which
    /// is how a recursive call is seen: as a call to something unknown.
    fn summary(&self, index: usize) -> Option<Rc<Summary>> {
        if let Some(cached) = self.summaries.borrow().get(&index) {
            return cached.clone();
        }
        self.summaries.borrow_mut().insert(index, None);

        let info = &self.fns[index];
        let params = info.params();
        let mut ex = Extractor::new(self, info);
        for (name, kind, _) in &params {
            match (name, kind) {
                (Some(name), ParamKind::Account) => ex.add_binding(name.clone(), Origin::Param),
                (Some(name), ParamKind::Bytes) => {
                    ex.bytes.insert(name.clone(), false);
                }
                _ => {}
            }
        }
        ex.visit_block(info.block);
        ex.apply_macro_validations();

        let params = params
            .iter()
            .map(|(name, kind, _)| match (name, kind) {
                (Some(name), ParamKind::Account) => {
                    let index = ex.index[name];
                    let binding = &ex.bindings[index];
                    ParamSummary::Account {
                        validations: binding.validations.clone(),
                        uses: binding.uses.clone(),
                        delegated: binding.delegated,
                        authority: binding.authority_site.is_some(),
                        invoked_as_program: ex
                            .cpi_sites
                            .iter()
                            .any(|site| site.program_binding == Some(index)),
                    }
                }
                (Some(name), ParamKind::Bytes) => ParamSummary::Bytes {
                    len_checked: ex.bytes.get(name).copied().unwrap_or(false),
                },
                _ => ParamSummary::Other,
            })
            .collect();
        let summary = Rc::new(Summary { params });
        self.summaries
            .borrow_mut()
            .insert(index, Some(summary.clone()));
        Some(summary)
    }

    fn returns_account(&self, index: usize) -> bool {
        struct Finder(bool);
        impl<'ast> Visit<'ast> for Finder {
            fn visit_path_segment(&mut self, segment: &'ast syn::PathSegment) {
                if ACCOUNT_SLICE_TYPES.contains(&segment.ident.to_string().as_str()) {
                    self.0 = true;
                }
                syn::visit::visit_path_segment(self, segment);
            }
        }
        let mut finder = Finder(false);
        finder.visit_return_type(&self.fns[index].sig.output);
        finder.0
    }

    /// The structs with a field of type `context`.
    fn holders_of(&self, context: &str) -> Vec<String> {
        let mut holders: Vec<String> = self
            .structs
            .iter()
            .filter(|(_, fields)| fields.iter().any(|(_, ty)| ty == context))
            .map(|(name, _)| name.clone())
            .collect();
        holders.sort();
        holders
    }
}

/// `src/token/account.rs` is `["token", "account"]`; `mod.rs`, `lib.rs` and
/// `main.rs` name their directory.
fn module_path(path: &std::path::Path) -> Vec<String> {
    let mut parts: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .skip_while(|c| c != "src")
        .skip(1)
        .collect();
    if let Some(last) = parts.pop() {
        let stem = last.trim_end_matches(".rs");
        if !matches!(stem, "mod" | "lib" | "main") {
            parts.push(stem.to_string());
        }
    }
    parts
}

/// Builds a fact table for every instruction handler in the crate. A handler
/// is a free function, `impl` method, or trait default method taking a
/// `&[AccountInfo]`/`&[AccountView]` slice.
pub fn analyze(files: &[ParsedFile]) -> Vec<Handler> {
    let analyzer = Analyzer::new(files);
    let mut handlers = Vec::new();

    for (index, info) in analyzer.fns.iter().enumerate() {
        let Some(accounts_param) = accounts_param_name(info.sig) else {
            continue;
        };
        let mut ex = Extractor::new(&analyzer, info);
        ex.accounts_param = Some(accounts_param);
        // A method may build its own type and keep using it (`let loaded =
        // Self { .. }; loaded.vault`), so the type is the context from the start.
        ex.context = info.owner.clone();
        ex.visit_block(info.block);
        ex.apply_macro_validations();

        // Follow the accounts this handler stored in its own type.
        let mut followed_by = Vec::new();
        if let (Some(context), false) = (info.owner.clone(), ex.stored.is_empty()) {
            let holders = analyzer.holders_of(&context);
            for (other, consumer) in analyzer.fns.iter().enumerate() {
                if other == index {
                    continue;
                }
                let roots = consumer_roots(consumer, &context, &holders);
                if roots.is_empty() && !constructs(consumer.block, &context, &holders) {
                    continue;
                }
                ex.enter(consumer, roots);
                ex.visit_block(consumer.block);
                ex.apply_macro_validations();
                followed_by.push(consumer.display());
            }
        }

        handlers.push(Handler {
            name: info.display(),
            file: info.file.clone(),
            bindings: ex.bindings,
            cpi_sites: ex.cpi_sites,
            stores_accounts: ex.stores_accounts,
            followed_by,
        });
    }
    handlers
}

/// The names in `consumer` that hold the context or a struct containing it:
/// `self`, and parameters typed as either.
fn consumer_roots(consumer: &FnInfo, context: &str, holders: &[String]) -> Vec<(String, Root)> {
    let root_for = |ty: &str| {
        if ty == context {
            Some(Root::Context)
        } else if holders.iter().any(|h| h == ty) {
            Some(Root::Holder(ty.to_string()))
        } else {
            None
        }
    };
    let mut roots = Vec::new();
    if consumer.has_receiver() {
        if let Some(root) = consumer.owner.as_deref().and_then(root_for) {
            roots.push(("self".to_string(), root));
        }
    }
    for (name, _, ty) in consumer.params() {
        if let (Some(name), Some(root)) = (name, ty.as_deref().and_then(root_for)) {
            roots.push((name, root));
        }
    }
    roots
}

/// Whether `block` builds the context or a holder through an associated
/// function (`Deposit::try_from(..)`), which gives a local of that type.
fn constructs(block: &syn::Block, context: &str, holders: &[String]) -> bool {
    struct Finder<'a> {
        context: &'a str,
        holders: &'a [String],
        found: bool,
    }
    impl<'ast> Visit<'ast> for Finder<'_> {
        fn visit_local(&mut self, local: &'ast syn::Local) {
            if let Some(init) = &local.init {
                if constructed_type(&init.expr)
                    .is_some_and(|ty| ty == self.context || self.holders.contains(&ty))
                {
                    self.found = true;
                }
            }
            syn::visit::visit_local(self, local);
        }
    }
    let mut finder = Finder {
        context,
        holders,
        found: false,
    };
    finder.visit_block(block);
    finder.found
}

/// `T` for an expression of the form `T::function(..)`, through `?`.
fn constructed_type(expr: &syn::Expr) -> Option<String> {
    let syn::Expr::Call(call) = strip_try(expr) else {
        return None;
    };
    call_path_tail(&call.func)?.0
}

/// The parameter name of the accounts slice, if the signature has one. Also matches a
/// tuple parameter such as `(data, accounts): (&[u8], &[AccountView])`.
fn accounts_param_name(sig: &syn::Signature) -> Option<String> {
    for input in &sig.inputs {
        let syn::FnArg::Typed(pat_ty) = input else {
            continue;
        };
        if let Some(name) = slice_param_name(&pat_ty.pat, &pat_ty.ty) {
            return Some(name);
        }
    }
    None
}

fn slice_param_name(pat: &syn::Pat, ty: &syn::Type) -> Option<String> {
    match (pat, ty) {
        (syn::Pat::Ident(p), _) if is_account_slice(ty) => Some(p.ident.to_string()),
        (syn::Pat::Tuple(pats), syn::Type::Tuple(tys)) => pats
            .elems
            .iter()
            .zip(&tys.elems)
            .find_map(|(p, t)| slice_param_name(p, t)),
        _ => None,
    }
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

/// What a name refers to while following a context: the context itself, or a
/// struct that has it as a field.
#[derive(Clone, PartialEq)]
enum Root {
    Context,
    Holder(String),
}

struct Extractor<'a, 'i> {
    analyzer: &'a Analyzer<'i>,
    file: String,
    /// The function being walked, for `read_in`.
    function: String,
    /// The `impl` type the function is a method of, if any.
    owner: Option<String>,
    accounts_param: Option<String>,
    bindings: Vec<AccountBinding>,
    /// Local names of the bindings, in the function being walked.
    index: HashMap<String, usize>,
    macro_texts: Vec<(String, String)>,
    /// `let ix = InstructionView { … }` local name → its program-id source.
    instr_programs: HashMap<String, Option<usize>>,
    cpi_sites: Vec<CpiSite>,
    /// `(field, binding)` for each account moved into the handler's own type.
    stored: Vec<(String, usize)>,
    stores_accounts: Option<Site>,
    /// The handler's own type, once its stored accounts are being followed.
    context: Option<String>,
    context_fields: HashMap<String, usize>,
    roots: HashMap<String, Root>,
    /// Byte-slice parameters, and whether their length is tested.
    bytes: HashMap<String, bool>,
    /// Locals holding an account's unchecked-borrowed data, and that account.
    byte_locals: HashMap<String, usize>,
}

impl<'a, 'i> Extractor<'a, 'i> {
    fn new(analyzer: &'a Analyzer<'i>, info: &FnInfo) -> Self {
        Extractor {
            analyzer,
            file: info.file.clone(),
            function: info.display(),
            owner: info.owner.clone(),
            accounts_param: None,
            bindings: Vec::new(),
            index: HashMap::new(),
            macro_texts: Vec::new(),
            instr_programs: HashMap::new(),
            cpi_sites: Vec::new(),
            stored: Vec::new(),
            stores_accounts: None,
            context: None,
            context_fields: HashMap::new(),
            roots: HashMap::new(),
            bytes: HashMap::new(),
            byte_locals: HashMap::new(),
        }
    }

    /// Starts walking another function against the same bindings. Its locals
    /// are its own; the accounts are reached through `roots`.
    fn enter(&mut self, info: &FnInfo, roots: Vec<(String, Root)>) {
        self.file = info.file.clone();
        self.function = info.display();
        self.owner = info.owner.clone();
        self.accounts_param = None;
        self.index.clear();
        self.instr_programs.clear();
        self.bytes.clear();
        self.byte_locals.clear();
        self.roots = roots.into_iter().collect();
    }

    fn site(&self, span: proc_macro2::Span) -> Site {
        Site {
            file: self.file.clone(),
            span,
        }
    }

    fn add_binding(&mut self, name: String, origin: Origin) {
        if self.index.contains_key(&name) {
            return;
        }
        self.index.insert(name.clone(), self.bindings.len());
        self.bindings.push(AccountBinding::new(name, origin));
    }

    /// The binding an expression names: a local, or a field of the context
    /// reached through `self`, a parameter, or a local of that type.
    fn resolve(&self, expr: &syn::Expr) -> Option<usize> {
        match peel(expr) {
            syn::Expr::Path(p) => self.index.get(&p.path.get_ident()?.to_string()).copied(),
            syn::Expr::Field(f) => {
                let syn::Member::Named(member) = &f.member else {
                    return None;
                };
                (self.root_of(&f.base)? == Root::Context)
                    .then(|| self.context_fields.get(&member.to_string()).copied())
                    .flatten()
            }
            _ => None,
        }
    }

    fn root_of(&self, expr: &syn::Expr) -> Option<Root> {
        match peel(expr) {
            syn::Expr::Path(p) => self.roots.get(&p.path.get_ident()?.to_string()).cloned(),
            syn::Expr::Field(f) => {
                let Root::Holder(holder) = self.root_of(&f.base)? else {
                    return None;
                };
                let syn::Member::Named(member) = &f.member else {
                    return None;
                };
                let fields = self.analyzer.structs.get(&holder)?;
                let (_, ty) = fields.iter().find(|(name, _)| member == name)?;
                self.root_for_type(ty)
            }
            _ => None,
        }
    }

    fn root_for_type(&self, ty: &str) -> Option<Root> {
        let context = self.context.as_deref()?;
        if ty == context {
            Some(Root::Context)
        } else if self
            .analyzer
            .structs
            .get(ty)
            .is_some_and(|fields| fields.iter().any(|(_, field_ty)| field_ty == context))
        {
            Some(Root::Holder(ty.to_string()))
        } else {
            None
        }
    }

    /// The binding whose data an expression borrows unchecked, through `unsafe`.
    fn unchecked_borrow_of(&self, expr: &syn::Expr) -> Option<usize> {
        match peel(expr) {
            syn::Expr::Unsafe(u) => match u.block.stmts.as_slice() {
                [syn::Stmt::Expr(inner, None)] => self.unchecked_borrow_of(inner),
                _ => None,
            },
            syn::Expr::MethodCall(m) => match borrow_use_for_method(&m.method.to_string()) {
                Some(Use::BorrowData {
                    unchecked: true, ..
                }) => self.resolve(&m.receiver),
                _ => None,
            },
            _ => None,
        }
    }

    /// Whether an expression contains a struct literal of the function's own type.
    fn builds_own_type(&self, expr: &syn::Expr) -> bool {
        struct Finder<'a>(Option<&'a str>, bool);
        impl<'ast> Visit<'ast> for Finder<'_> {
            fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
                if node.path.segments.last().is_some_and(|seg| {
                    seg.ident == "Self" || self.0.is_some_and(|ty| seg.ident == ty)
                }) {
                    self.1 = true;
                }
                syn::visit::visit_expr_struct(self, node);
            }
        }
        if self.owner.is_none() || self.accounts_param.is_none() {
            return false;
        }
        let mut finder = Finder(self.owner.as_deref(), false);
        finder.visit_expr(expr);
        finder.1
    }

    fn note_read(&mut self, idx: usize, span: proc_macro2::Span) {
        if self.bindings[idx].read_site.is_none() {
            self.bindings[idx].read_site = Some(self.site(span));
            self.bindings[idx].read_in = Some(self.function.clone());
        }
    }

    fn apply_macro_validations(&mut self) {
        // Owner/signer/key checks are often written inside macros (`assert_eq!`,
        // `require!`), whose bodies syn does not visit as expressions. Scan each
        // macro's token text for `<binding> . method`, skipping log/format macros
        // where a field mention is not a check.
        let texts = std::mem::take(&mut self.macro_texts);
        for i in 0..self.bindings.len() {
            let name = self.bindings[i].name.clone();
            for (macro_name, text) in &texts {
                if LOG_MACROS.contains(&macro_name.as_str()) {
                    continue;
                }
                for (method, val) in [
                    ("owner", Validation::Owner),
                    ("owned_by", Validation::Owner),
                    ("is_owned_by", Validation::Owner),
                    ("key", Validation::Key),
                    ("address", Validation::Key),
                    // A key in a macro is almost always a comparison.
                    ("key", Validation::KeyCompared),
                    ("address", Validation::KeyCompared),
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

impl<'ast> Visit<'ast> for Extractor<'_, '_> {
    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let Some(init) = &local.init {
            self.discover_binding(&local.pat, &init.expr);
            if let Some(name) = local_name(&local.pat) {
                // Record a `let ix = InstructionView { … }` so a later invoke resolves it.
                if let Some(s) = as_instruction_struct(&init.expr) {
                    let src = self.struct_program_binding(s);
                    self.instr_programs.insert(name.clone(), src);
                }
                // A local that holds the context, or a struct containing it.
                let root = self
                    .root_of(strip_try(&init.expr))
                    .or_else(|| constructed_type(&init.expr).and_then(|ty| self.root_for_type(&ty)))
                    .or_else(|| self.builds_own_type(&init.expr).then_some(Root::Context));
                // `let data = unsafe { account.borrow_unchecked() }`.
                if let Some(idx) = self.unchecked_borrow_of(strip_try(&init.expr)) {
                    self.byte_locals.insert(name.clone(), idx);
                }
                if let Some(root) = root {
                    self.roots.insert(name, root);
                }
            }
        }
        syn::visit::visit_local(self, local);
    }

    fn visit_expr_binary(&mut self, node: &'ast syn::ExprBinary) {
        if matches!(node.op, syn::BinOp::Eq(_) | syn::BinOp::Ne(_)) {
            self.note_comparison(&node.left, &node.right, node.span());
        }
        syn::visit::visit_expr_binary(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if method == "eq" || method == "ne" {
            if let Some(arg) = node.args.first() {
                self.note_comparison(&node.receiver, arg, node.span());
            }
        }
        if method == "len" {
            if let syn::Expr::Path(p) = peel(&node.receiver) {
                let name = p
                    .path
                    .get_ident()
                    .map(|id| id.to_string())
                    .unwrap_or_default();
                if let Some(checked) = self.bytes.get_mut(&name) {
                    *checked = true;
                }
                if let Some(idx) = self.byte_locals.get(&name).copied() {
                    self.bindings[idx]
                        .validations
                        .insert(Validation::LengthChecked);
                }
            }
        }
        if let Some(idx) = self.resolve(&node.receiver) {
            if let Some(v) = validation_for_method(&method) {
                self.bindings[idx].validations.insert(v);
            } else if let Some(u) = borrow_use_for_method(&method) {
                self.note_read(idx, node.method.span());
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

    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        // `Self { vault, authority }` in a method: the accounts leave this function
        // inside the handler's own type.
        let is_own_type = node.path.segments.last().is_some_and(|seg| {
            seg.ident == "Self" || self.owner.as_deref().is_some_and(|ty| seg.ident == ty)
        });
        let is_create = node
            .path
            .segments
            .last()
            .is_some_and(|seg| CREATE_BUILDERS.contains(&seg.ident.to_string().as_str()));
        if is_create {
            for field in &node.fields {
                if matches!(&field.member, syn::Member::Named(m) if m == "to") {
                    if let Some(idx) = self.resolve(&field.expr) {
                        self.bindings[idx].validations.insert(Validation::Created);
                    }
                }
            }
        }
        if is_own_type && self.owner.is_some() && self.accounts_param.is_some() {
            for field in &node.fields {
                let (syn::Member::Named(member), Some(idx)) =
                    (&field.member, self.resolve(&field.expr))
                else {
                    continue;
                };
                self.stored.push((member.to_string(), idx));
                self.context_fields.insert(member.to_string(), idx);
                if self.stores_accounts.is_none() {
                    self.stores_accounts = Some(self.site(node.span()));
                }
            }
        }
        syn::visit::visit_expr_struct(self, node);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let name = mac
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();
        self.macro_texts.push((name, mac.tokens.to_string()));
        syn::visit::visit_macro(self, mac);
    }
}

fn local_name(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(p) => Some(p.ident.to_string()),
        syn::Pat::Type(t) => local_name(&t.pat),
        _ => None,
    }
}

impl Extractor<'_, '_> {
    fn discover_binding(&mut self, pat: &syn::Pat, init: &syn::Expr) {
        match pat {
            // `let [a, b, ..] = accounts else { ... }`
            syn::Pat::Slice(slice) if self.is_accounts_expr(init) => {
                for elem in &slice.elems {
                    if let syn::Pat::Ident(p) = elem {
                        // `rest @ ..` binds the remaining slice, not an account.
                        if p.subpat.is_none() {
                            self.add_binding(p.ident.to_string(), Origin::SlicePattern);
                        }
                    }
                }
            }
            syn::Pat::Ident(p) => {
                let name = p.ident.to_string();
                if self.is_index_of_accounts(init) || self.is_account_from_helper(init) {
                    self.add_binding(name, Origin::Index);
                } else if self.is_iter_next(init) {
                    self.add_binding(name, Origin::Iter);
                } else if let Some(src) = self.resolve(init) {
                    // `let y = x;` renames binding x.
                    self.index.insert(name, src);
                }
            }
            _ => {}
        }
    }

    fn is_accounts_expr(&self, expr: &syn::Expr) -> bool {
        let Some(param) = self.accounts_param.as_deref() else {
            return false;
        };
        matches!(expr, syn::Expr::Path(p) if p.path.is_ident(param))
    }

    fn is_index_of_accounts(&self, expr: &syn::Expr) -> bool {
        let expr = strip_ref(expr);
        match expr {
            syn::Expr::Index(idx) => self.is_accounts_expr(&idx.expr),
            syn::Expr::MethodCall(m) => m.method == "get" && self.is_accounts_expr(&m.receiver),
            _ => false,
        }
    }

    /// `let a = next(accounts, 0)?`: a crate function that is handed the slice
    /// and returns an account.
    fn is_account_from_helper(&self, expr: &syn::Expr) -> bool {
        let syn::Expr::Call(call) = strip_try(expr) else {
            return false;
        };
        if !call
            .args
            .iter()
            .any(|arg| self.is_accounts_expr(strip_ref(arg)))
        {
            return false;
        }
        call_path(&call.func)
            .and_then(|segments| {
                self.analyzer
                    .resolve_callee(&segments, self.owner.as_deref())
            })
            .is_some_and(|callee| self.analyzer.returns_account(callee))
    }

    fn is_iter_next(&self, expr: &syn::Expr) -> bool {
        let expr = strip_try(expr);
        matches!(expr, syn::Expr::Call(c)
            if last_call_segment(&c.func).is_some_and(|s| ACCOUNT_ITER_FUNCS.contains(&s.as_str())))
    }

    fn classify_call(&mut self, node: &syn::ExprCall) {
        let Some(segments) = call_path(&node.func) else {
            // Not a path call; nothing to classify.
            return;
        };
        let fn_seg = segments[segments.len() - 1].clone();
        let type_seg = (segments.len() >= 2).then(|| segments[segments.len() - 2].clone());

        // `invoke`/`invoke_signed(...)`: mark CPI accounts, record the callee.
        if type_seg.is_none() && CPI_FUNCS.contains(&fn_seg.as_str()) {
            for arg in &node.args {
                if let Some(idx) = self.resolve(arg) {
                    self.bindings[idx].uses.insert(Use::CpiAccount);
                }
            }
            let program_binding = node.args.first().and_then(|a| self.cpi_program_binding(a));
            self.cpi_sites.push(CpiSite {
                program_binding,
                site: self.site(node.span()),
            });
            return;
        }

        let is_loader = type_seg.as_ref().is_some_and(|ty| {
            LOADER_METHODS.contains(&fn_seg.as_str()) && !SYSVAR_TYPES.contains(&ty.as_str())
        });

        // A function defined in this crate: apply what its body does.
        let summary = self
            .analyzer
            .resolve_callee(&segments, self.owner.as_deref())
            .and_then(|callee| {
                let skip_receiver = self.analyzer.fns[callee].has_receiver();
                Some((self.analyzer.summary(callee)?, skip_receiver))
            });
        if let Some((summary, skip_receiver)) = summary {
            let args = node.args.iter().skip(usize::from(skip_receiver));
            for (arg, param) in args.zip(&summary.params) {
                self.apply_param(arg, param, node.span());
            }
        } else {
            // Anything else is opaque: bytes it is given are its responsibility
            // to bound, and an account it is given is delegated, unless it is
            // a loader, which reads the account as state (below).
            for arg in &node.args {
                if !(is_loader && self.resolve(arg).is_some()) {
                    self.apply_param(arg, &ParamSummary::Other, node.span());
                }
            }
        }

        // `Type::loader(account)`: reading the account as program state.
        if let (true, Some(ty)) = (is_loader, &type_seg) {
            for arg in &node.args {
                if let Some(idx) = self.resolve(arg) {
                    self.note_read(idx, node.span());
                    self.bindings[idx]
                        .uses
                        .insert(Use::DeserializeState(ty.clone()));
                }
            }
        }
    }

    /// Applies what a callee does with one parameter to the argument passed for it.
    fn apply_param(&mut self, arg: &syn::Expr, param: &ParamSummary, call: proc_macro2::Span) {
        if let Some(idx) = self.resolve(arg) {
            let ParamSummary::Account {
                validations,
                uses,
                delegated,
                authority,
                invoked_as_program,
            } = param
            else {
                self.bindings[idx].delegated = true;
                return;
            };
            if uses
                .iter()
                .any(|u| matches!(u, Use::BorrowData { .. } | Use::DeserializeState(_)))
            {
                self.note_read(idx, call);
            }
            let site = self.site(call);
            let binding = &mut self.bindings[idx];
            binding.validations.extend(validations.iter().cloned());
            binding.uses.extend(uses.iter().cloned());
            binding.delegated |= delegated;
            if *authority && binding.authority_site.is_none() {
                binding.authority_site = Some(site.clone());
            }
            if *invoked_as_program {
                self.cpi_sites.push(CpiSite {
                    program_binding: Some(idx),
                    site,
                });
            }
        } else if let Some(idx) = self.unchecked_borrow_of(arg) {
            // The bytes go straight to the callee, which bounds them or does not.
            if !matches!(param, ParamSummary::Bytes { len_checked: false }) {
                self.bindings[idx]
                    .validations
                    .insert(Validation::LengthChecked);
            }
        } else if let syn::Expr::Path(p) = peel(arg) {
            let name = p
                .path
                .get_ident()
                .map(|id| id.to_string())
                .unwrap_or_default();
            let checked = !matches!(param, ParamSummary::Bytes { len_checked: false });
            if let Some(own) = self.bytes.get_mut(&name) {
                *own |= checked;
            }
            // A local of borrowed bytes counts as bounded only when a function
            // in this crate is seen to bound it. Handing it to anything else
            // leaves the borrow as it was.
            if let (Some(idx), ParamSummary::Bytes { len_checked: true }) =
                (self.byte_locals.get(&name).copied(), param)
            {
                self.bindings[idx]
                    .validations
                    .insert(Validation::LengthChecked);
            }
        }
    }

    /// Record a key comparison `a <cmp> b`: mark the key side `KeyCompared`, and if
    /// the other side names an authority field/var, mark it an authority position.
    fn note_comparison(&mut self, a: &syn::Expr, b: &syn::Expr, span: proc_macro2::Span) {
        for (side, other) in [(a, b), (b, a)] {
            if let Some(idx) = self.key_call_binding(side) {
                self.bindings[idx]
                    .validations
                    .insert(Validation::KeyCompared);
                if self.bindings[idx].authority_site.is_none() && is_authority_ref(other) {
                    self.bindings[idx].authority_site = Some(self.site(span));
                }
            }
        }
    }

    /// Binding index of `<account>.key()`/`.address()`, else `None`.
    fn key_call_binding(&self, expr: &syn::Expr) -> Option<usize> {
        if let syn::Expr::MethodCall(m) = peel(expr) {
            let method = m.method.to_string();
            if method == "key" || method == "address" {
                return self.resolve(&m.receiver);
            }
        }
        None
    }

    /// Program-id source of an invoke's first arg (inline struct or a local).
    fn cpi_program_binding(&self, arg0: &syn::Expr) -> Option<usize> {
        if let Some(s) = as_instruction_struct(arg0) {
            return self.struct_program_binding(s);
        }
        if let syn::Expr::Path(p) = peel(arg0) {
            if let Some(id) = p.path.get_ident() {
                return self.instr_programs.get(&id.to_string()).copied().flatten();
            }
        }
        None
    }

    /// The account binding feeding an instruction struct's `program_id` field.
    fn struct_program_binding(&self, s: &syn::ExprStruct) -> Option<usize> {
        for fv in &s.fields {
            if let syn::Member::Named(id) = &fv.member {
                if id == "program_id" {
                    return self.key_call_binding(&fv.expr);
                }
            }
        }
        None
    }
}

/// The segments of a path call's function, e.g. `["guards", "config"]`.
fn call_path(func: &syn::Expr) -> Option<Vec<String>> {
    let syn::Expr::Path(p) = func else {
        return None;
    };
    let segments: Vec<String> = p
        .path
        .segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect();
    (!segments.is_empty()).then_some(segments)
}

fn strip_ref(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::Reference(r) => strip_ref(&r.expr),
        syn::Expr::Paren(p) => strip_ref(&p.expr),
        _ => expr,
    }
}

/// Peels references, parens, groups, and derefs to the inner expression.
fn peel(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::Reference(r) => peel(&r.expr),
        syn::Expr::Paren(p) => peel(&p.expr),
        syn::Expr::Group(g) => peel(&g.expr),
        syn::Expr::Unary(syn::ExprUnary {
            op: syn::UnOp::Deref(_),
            expr,
            ..
        }) => peel(expr),
        _ => expr,
    }
}

/// Whether `expr` names an authority field/var (`state.authority`, `mint_auth`), the
/// value an account's key is compared against.
fn is_authority_ref(expr: &syn::Expr) -> bool {
    match peel(expr) {
        syn::Expr::Field(f) => {
            matches!(&f.member, syn::Member::Named(id) if is_authority_name(&id.to_string()))
        }
        syn::Expr::MethodCall(m) => is_authority_name(&m.method.to_string()),
        syn::Expr::Path(p) => p
            .path
            .segments
            .last()
            .is_some_and(|s| is_authority_name(&s.ident.to_string())),
        _ => false,
    }
}

fn is_authority_name(name: &str) -> bool {
    name == "authority"
        || name == "admin"
        || name == "auth"
        || name.ends_with("_authority")
        || name.ends_with("_auth")
}

/// The struct literal if `expr` is an `Instruction`/`InstructionView { … }`.
fn as_instruction_struct(expr: &syn::Expr) -> Option<&syn::ExprStruct> {
    if let syn::Expr::Struct(s) = peel(expr) {
        let last = s.path.segments.last()?;
        if INSTRUCTION_TYPES.contains(&last.ident.to_string().as_str()) {
            return Some(s);
        }
    }
    None
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
