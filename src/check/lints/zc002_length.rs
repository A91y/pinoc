use crate::check::contract::{Backend, Category, Confidence, Finding, Lint, ParsedFile, Severity};
use crate::check::facts::{extract_handlers, Validation};
use crate::check::lints::to_span;

/// Account data borrowed through an `*_unchecked` accessor with no `data_len()`
/// guard, so a shorter-than-expected account is read past its end.
pub struct Zc002Length;

impl Lint for Zc002Length {
    fn code(&self) -> &'static str {
        "ZC002-P"
    }
    fn id(&self) -> &'static str {
        "unchecked-length-before-cast"
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
        Confidence::Likely
    }

    fn run(&self, file: &ParsedFile) -> Vec<Finding> {
        let path = file.path.display().to_string();
        let mut out = Vec::new();
        for handler in extract_handlers(&file.ast) {
            for b in &handler.bindings {
                if b.delegated
                    || !b.unchecked_read()
                    || b.validations.contains(&Validation::LengthChecked)
                {
                    continue;
                }
                let Some(span) = b.read_span else {
                    continue;
                };
                out.push(Finding {
                    code: self.code(),
                    id: self.id(),
                    confidence: self.default_confidence(),
                    severity: self.default_severity(),
                    span: to_span(span, &path),
                    evidence: format!(
                        "account `{}` is borrowed unchecked without a `data_len()` guard; a shorter account reads past its end",
                        b.name
                    ),
                    fix: Some(format!(
                        "check `{}.data_len() >= size_of::<T>()` before the unchecked borrow",
                        b.name
                    )),
                });
            }
        }
        out
    }
}
