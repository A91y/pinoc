use crate::check::contract::{Backend, Category, Confidence, Finding, Lint, ParsedFile, Severity};
use crate::check::facts::{extract_handlers, Validation};
use crate::check::lints::to_span;

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

    fn run(&self, file: &ParsedFile) -> Vec<Finding> {
        let path = file.path.display().to_string();
        let mut out = Vec::new();
        for handler in extract_handlers(&file.ast) {
            for b in &handler.bindings {
                if b.delegated || !b.reads_data() || b.validations.contains(&Validation::Owner) {
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
                        "account `{}` is read as state without checking its owner; an attacker can pass a look-alike account",
                        b.name
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
