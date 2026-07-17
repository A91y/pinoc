use crate::check::contract::{Backend, Category, Confidence, Finding, Lint, ParsedFile, Severity};
use crate::check::lints::{item_top_span, to_span};
use crate::idl::padding_lint::{derives_shank_idl, is_repr_c, is_repr_transparent};
use syn::Item;

/// A struct read zero-copy without `#[repr(C)]`, so Rust's default layout may
/// reorder fields and break the byte mapping the client relies on.
pub struct Zc003ReprC;

impl Lint for Zc003ReprC {
    fn code(&self) -> &'static str {
        "ZC003-P"
    }
    fn id(&self) -> &'static str {
        "missing-repr-c"
    }
    fn category(&self) -> Category {
        Category::Zc
    }
    fn backend(&self) -> Backend {
        Backend::Syn
    }
    fn default_severity(&self) -> Severity {
        Severity::Deny
    }
    fn default_confidence(&self) -> Confidence {
        Confidence::Definite
    }

    fn run(&self, file: &ParsedFile) -> Vec<Finding> {
        let path = file.path.display().to_string();
        let mut out = Vec::new();
        for item in &file.ast.items {
            let Item::Struct(s) = item else {
                continue;
            };
            if !derives_shank_idl(s) || is_repr_c(s) || is_repr_transparent(s) {
                continue;
            }
            out.push(Finding {
                code: self.code(),
                id: self.id(),
                confidence: self.default_confidence(),
                severity: self.default_severity(),
                span: to_span(item_top_span(&s.attrs, s.ident.span()), &path),
                evidence: format!(
                    "`{}` derives ShankAccount/ShankType (read zero-copy) but is not #[repr(C)]; the default layout may reorder fields",
                    s.ident
                ),
                fix: Some("add #[repr(C)]".to_string()),
            });
        }
        out
    }
}
