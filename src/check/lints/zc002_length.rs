use crate::check::contract::{Backend, Category, Confidence, Finding, Lint, Severity};
use crate::check::facts::{Handler, Validation};

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

    fn run_handlers(&self, handlers: &[Handler]) -> Vec<Finding> {
        let mut out = Vec::new();
        for handler in handlers {
            for b in &handler.bindings {
                if b.delegated
                    || !b.unchecked_read()
                    || b.validations.contains(&Validation::LengthChecked)
                {
                    continue;
                }
                let Some(site) = &b.read_site else {
                    continue;
                };
                out.push(Finding {
                    code: self.code(),
                    id: self.id(),
                    confidence: self.default_confidence(),
                    severity: self.default_severity(),
                    span: site.to_span(),
                    evidence: format!(
                        "account `{}` is borrowed unchecked without a `data_len()` guard; a shorter account reads past its end{}",
                        b.name,
                        handler.used_in(b)
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
