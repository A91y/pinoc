use crate::check::contract::{Backend, Category, Confidence, Finding, Lint, ParsedFile, Severity};
use crate::check::facts::{extract_handlers, Validation};
use crate::check::lints::to_span;
use std::collections::HashSet;

/// `invoke`/`invoke_signed` to a program from a caller-supplied account whose key
/// is never compared to an expected id, letting an attacker redirect the call.
pub struct Cpi001ArbitraryCpi;

impl Lint for Cpi001ArbitraryCpi {
    fn code(&self) -> &'static str {
        "CPI001-P"
    }
    fn id(&self) -> &'static str {
        "arbitrary-cpi"
    }
    fn category(&self) -> Category {
        Category::Cpi
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
            let mut seen = HashSet::new();
            for site in &handler.cpi_sites {
                let Some(idx) = site.program_binding else {
                    continue;
                };
                let b = &handler.bindings[idx];
                if b.delegated || b.validations.contains(&Validation::KeyCompared) {
                    continue;
                }
                if !seen.insert(idx) {
                    continue;
                }
                out.push(Finding {
                    code: self.code(),
                    id: self.id(),
                    confidence: self.default_confidence(),
                    severity: self.default_severity(),
                    span: to_span(site.span, &path),
                    evidence: format!(
                        "account `{}` is invoked as a program without checking its key against an expected id; an attacker can point it at malicious code",
                        b.name
                    ),
                    fix: Some(format!(
                        "compare `{}.key()` to the expected program id before invoking",
                        b.name
                    )),
                });
            }
        }
        out
    }
}
