use crate::check::contract::{Backend, Category, Confidence, Finding, Lint, Severity};
use crate::check::facts::Handler;

/// An account read as this program's state without checking `owner() ==
/// program_id`, letting an attacker pass a look-alike account they control.
pub struct Acc001Owner;

impl Lint for Acc001Owner {
    fn code(&self) -> &'static str {
        "ACC001-P"
    }
    fn id(&self) -> &'static str {
        "missing-owner"
    }
    fn category(&self) -> Category {
        Category::Acc
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
                if b.delegated || !b.reads_data() || b.identity_established() {
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
                        "account `{}` is read as state without checking its owner; an attacker can pass a look-alike account{}",
                        b.name,
                        handler.used_in(b)
                    ),
                    fix: Some(format!(
                        "check `{}.owner() == program_id` before reading its data",
                        b.name
                    )),
                });
            }
        }
        out
    }
}
