//! Frozen contract for `pinoc check`. Codes, ids, and the JSON shape of `Finding`
//! are a permanent compatibility surface once released; do not rename.
//!
//! Some of this surface has no caller until the first lint is registered.
#![allow(dead_code)]

use serde::Serialize;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Deny,
    Warn,
    Info,
}

/// Ordered weakest to strongest, so `confidence >= threshold` works directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Heuristic,
    Likely,
    Definite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Acc,
    Cpi,
    Zc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Syn,
    Dylint,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Span {
    pub file: String,
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Finding {
    pub code: &'static str,
    pub id: &'static str,
    pub confidence: Confidence,
    pub severity: Severity,
    pub span: Span,
    pub evidence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

/// One program source file, parsed once and shared across every lint.
pub struct ParsedFile {
    pub path: PathBuf,
    pub src: String,
    pub ast: syn::File,
}

pub trait Lint {
    fn code(&self) -> &'static str;
    fn id(&self) -> &'static str;
    fn category(&self) -> Category;
    fn backend(&self) -> Backend;
    fn default_severity(&self) -> Severity;
    fn default_confidence(&self) -> Confidence;
    /// Raw findings for one file, before config severity and suppression.
    fn run(&self, file: &ParsedFile) -> Vec<Finding>;
}
