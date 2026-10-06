use crate::check::contract::{Backend, Category, Confidence, Finding, Lint, Severity};
use crate::check::facts::{Handler, Validation};

/// An account named like a known singleton (`config`/`settings`) read as trusted
/// state without checking its key, letting an attacker substitute a look-alike.
pub struct Acc003Confusion;

impl Lint for Acc003Confusion {
    fn code(&self) -> &'static str {
        "ACC003-P"
    }
    fn id(&self) -> &'static str {
        "account-confusion"
    }
    fn category(&self) -> Category {
        Category::Acc
    }
    fn backend(&self) -> Backend {
        Backend::Syn
    }
    fn default_severity(&self) -> Severity {
        Severity::Warn
    }
    fn default_confidence(&self) -> Confidence {
        Confidence::Heuristic
    }

    fn run_handlers(&self, handlers: &[Handler]) -> Vec<Finding> {
        let mut out = Vec::new();
        for handler in handlers {
            for b in &handler.bindings {
                if !is_identity_name(&b.name) {
                    continue;
                }
                if b.delegated
                    || !b.reads_data()
                    || b.validations.contains(&Validation::KeyCompared)
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
                        "account `{}` is read as a known singleton but its key is never checked against the expected address; an attacker can pass a look-alike account",
                        b.name
                    ),
                    fix: Some(format!(
                        "compare `{}.key()` to the expected address (or the derived PDA) before reading it",
                        b.name
                    )),
                });
            }
        }
        out
    }
}

fn is_identity_name(name: &str) -> bool {
    name == "config"
        || name == "settings"
        || name.ends_with("_config")
        || name.ends_with("_settings")
}
