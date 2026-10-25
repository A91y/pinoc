//! Every code `pinoc check` can report: the registered lints, then the coverage
//! and meta codes the orchestrator emits itself. One list, so config
//! validation, `--explain` and SARIF rules cannot disagree.

use crate::check::contract::{Category, Confidence, Severity};
use crate::check::lints;

pub const NO_HANDLERS: &str = "NO-HANDLERS";
pub const UNTRACKED_ACCOUNTS: &str = "UNTRACKED-ACCOUNTS";
pub const UNUSED_ALLOW: &str = "UNUSED-ALLOW";

pub struct CodeInfo {
    pub code: &'static str,
    pub id: &'static str,
    /// `None` for coverage and meta codes, which are not lints.
    pub category: Option<Category>,
    pub default_severity: Severity,
    pub default_confidence: Confidence,
    pub summary: &'static str,
    pub explain: &'static str,
}

pub fn all() -> Vec<CodeInfo> {
    let mut out: Vec<CodeInfo> = lints::registry()
        .iter()
        .map(|l| CodeInfo {
            code: l.code(),
            id: l.id(),
            category: Some(l.category()),
            default_severity: l.default_severity(),
            default_confidence: l.default_confidence(),
            summary: l.summary(),
            explain: l.explain(),
        })
        .collect();
    out.extend([
        CodeInfo {
            code: NO_HANDLERS,
            id: "no-handlers",
            category: None,
            default_severity: Severity::Warn,
            default_confidence: Confidence::Definite,
            summary:
                "No instruction handler was found, so the account and CPI lints analysed nothing.",
            explain: include_str!("../../docs/lints/NO-HANDLERS.md"),
        },
        CodeInfo {
            code: UNTRACKED_ACCOUNTS,
            id: "untracked-accounts",
            category: None,
            default_severity: Severity::Warn,
            default_confidence: Confidence::Definite,
            summary: "Handlers whose accounts were not analysed, so they have no clean result.",
            explain: include_str!("../../docs/lints/UNTRACKED-ACCOUNTS.md"),
        },
        CodeInfo {
            code: UNUSED_ALLOW,
            id: "unused-allow",
            category: None,
            default_severity: Severity::Warn,
            default_confidence: Confidence::Definite,
            summary: "A pinoc:allow comment that matched no finding.",
            explain: include_str!("../../docs/lints/UNUSED-ALLOW.md"),
        },
    ]);
    out
}

/// Codes `--deny`, `--allow` and `[check]` accept. `UNUSED-ALLOW` is emitted
/// after configuration is applied, so naming it there would have no effect.
pub fn configurable() -> Vec<&'static str> {
    all()
        .into_iter()
        .map(|c| c.code)
        .filter(|c| *c != UNUSED_ALLOW)
        .collect()
}
