use crate::check::contract::{Backend, Category, Confidence, Finding, Lint, ParsedFile, Severity};
use crate::check::facts::{extract_handlers, Validation};
use crate::check::lints::to_span;

/// An account checked against a stored authority id but never verified as a
/// signer, so anyone can act as that authority by passing its public address.
pub struct Acc002Signer;

impl Lint for Acc002Signer {
    fn code(&self) -> &'static str {
        "ACC002-P"
    }
    fn id(&self) -> &'static str {
        "missing-signer"
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
        Confidence::Likely
    }

    fn run(&self, file: &ParsedFile) -> Vec<Finding> {
        let path = file.path.display().to_string();
        let mut out = Vec::new();
        for handler in extract_handlers(&file.ast) {
            for b in &handler.bindings {
                let Some(span) = b.authority_span else {
                    continue;
                };
                if b.delegated || b.validations.contains(&Validation::Signer) {
                    continue;
                }
                out.push(Finding {
                    code: self.code(),
                    id: self.id(),
                    confidence: self.default_confidence(),
                    severity: self.default_severity(),
                    span: to_span(span, &path),
                    evidence: format!(
                        "account `{}` is checked against a stored authority but never verified as a signer; anyone can pass this address without signing",
                        b.name
                    ),
                    fix: Some(format!(
                        "require `{}.is_signer()` before trusting it as the authority",
                        b.name
                    )),
                });
            }
        }
        out
    }
}
