use crate::check::contract::{Backend, Category, Confidence, Finding, Lint, ParsedFile, Severity};
use crate::check::lints::{item_top_span, to_span};
use crate::idl::padding_lint::{check_padding, derives_shank_idl, is_repr_c};
use syn::Item;

/// A `#[repr(C)]` zero-copy struct whose padded layout differs from its packed
/// borsh layout, so on-chain reads and client (de)serialization disagree.
pub struct Zc001Padding;

impl Lint for Zc001Padding {
    fn code(&self) -> &'static str {
        "ZC001-P"
    }
    fn id(&self) -> &'static str {
        "layout-padding-mismatch"
    }
    fn category(&self) -> Category {
        Category::Zc
    }
    fn backend(&self) -> Backend {
        Backend::Syn
    }
    fn default_severity(&self) -> Severity {
        // Only breaks a borsh client; a program driven another way is unaffected.
        Severity::Warn
    }
    fn default_confidence(&self) -> Confidence {
        Confidence::Likely
    }

    fn run(&self, file: &ParsedFile) -> Vec<Finding> {
        let path = file.path.display().to_string();
        let mut out = Vec::new();
        for item in &file.ast.items {
            let Item::Struct(s) = item else {
                continue;
            };
            if !(is_repr_c(s) && derives_shank_idl(s)) {
                continue;
            }
            let Some(w) = check_padding(s) else {
                continue;
            };
            let pad = w.repr_c_size - w.packed_size;
            out.push(Finding {
                code: self.code(),
                id: self.id(),
                confidence: self.default_confidence(),
                severity: self.default_severity(),
                span: to_span(item_top_span(&s.attrs, s.ident.span()), &path),
                evidence: format!(
                    "`{}` has {pad} byte(s) of implicit padding (layout {} vs packed {}); zero-copy reads won't match its packed borsh form",
                    w.name, w.repr_c_size, w.packed_size
                ),
                fix: Some(format!("add explicit `_padding: [u8; {pad}]` field(s)")),
            });
        }
        out
    }
}
