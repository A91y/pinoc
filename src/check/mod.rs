pub mod contract;
pub mod lints;
pub mod output;
pub mod suppress;

use crate::check::contract::{Confidence, Finding, ParsedFile, Severity, Span};
use crate::check::suppress::Suppressions;
use crate::config;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct CheckOptions {
    pub json: bool,
    pub deny: Vec<String>,
    pub allow: Vec<String>,
}

pub(crate) struct EffectiveConfig {
    pub deny: Vec<String>,
    pub warn: Vec<String>,
    pub allow: Vec<String>,
    pub threshold: Confidence,
}

pub fn run(opts: CheckOptions) -> Result<i32> {
    let cfg = load_effective_config(&opts)?;

    let lints = lints::registry();
    let known: Vec<&'static str> = lints.iter().map(|l| l.code()).collect();
    reject_unknown_codes(&cfg, &known)?;

    let src_dir = Path::new("src");
    let mut files = Vec::new();
    if src_dir.exists() {
        collect_rs_files(src_dir, &mut files)?;
    }

    let mut raw = Vec::new();
    let mut supp = Suppressions::default();
    for path in &files {
        let src = std::fs::read_to_string(path)?;
        supp.scan(&path.display().to_string(), &src);
        let Ok(ast) = syn::parse_file(&src) else {
            continue;
        };
        let parsed = ParsedFile {
            path: path.clone(),
            src,
            ast,
        };
        for lint in &lints {
            raw.extend(lint.run(&parsed));
        }
    }

    let (findings, exit_code) = process_findings(raw, &cfg, &mut supp);
    if opts.json {
        output::render_json(&findings)?;
    } else {
        output::render_human(&findings);
    }
    Ok(exit_code)
}

/// Applies config severity, inline suppression, and the confidence threshold to
/// raw findings; appends unused-allow findings; returns the survivors and the
/// exit code (nonzero iff any survivor is `Deny`).
pub(crate) fn process_findings(
    raw: Vec<Finding>,
    cfg: &EffectiveConfig,
    supp: &mut Suppressions,
) -> (Vec<Finding>, i32) {
    let mut out = Vec::new();
    for mut f in raw {
        if code_matches(&cfg.allow, f.code) {
            continue;
        }
        let denied = code_matches(&cfg.deny, f.code);
        if denied {
            f.severity = Severity::Deny;
        } else if code_matches(&cfg.warn, f.code) {
            f.severity = Severity::Warn;
        }
        if supp.is_suppressed(&f.span.file, f.span.line, f.code) {
            continue;
        }
        // A weak finding survives the threshold only when explicitly denied.
        if f.confidence < cfg.threshold && !denied {
            continue;
        }
        out.push(f);
    }

    for a in supp.unused() {
        out.push(Finding {
            code: "UNUSED-ALLOW",
            id: "unused-allow",
            confidence: Confidence::Definite,
            severity: Severity::Warn,
            span: Span {
                file: a.file.clone(),
                line: a.line,
                col: 0,
            },
            evidence: format!("`pinoc:allow({})` matched no finding", a.code),
            fix: None,
        });
    }

    let exit_code = i32::from(out.iter().any(|f| f.severity == Severity::Deny));
    (out, exit_code)
}

fn load_effective_config(opts: &CheckOptions) -> Result<EffectiveConfig> {
    let check = config::read_pinoc_config_optional()?
        .map(|c| c.check)
        .unwrap_or_default();
    let mut deny = check.deny;
    deny.extend(opts.deny.iter().cloned());
    let mut allow = check.allow;
    allow.extend(opts.allow.iter().cloned());
    let threshold = parse_confidence(check.confidence_threshold.as_deref());
    Ok(EffectiveConfig {
        deny,
        warn: check.warn,
        allow,
        threshold,
    })
}

/// A code list matches a finding's code exactly, or via `*`/`all` (every code).
fn code_matches(list: &[String], code: &str) -> bool {
    list.iter().any(|c| is_wildcard(c) || c == code)
}

fn is_wildcard(code: &str) -> bool {
    code == "*" || code == "all"
}

/// Rejects any deny/warn/allow value that is neither a real lint code nor
/// `*`/`all`. This blocks typos and a bare `--deny *` (which the shell expands
/// into filenames before pinoc runs) with one clear error.
fn reject_unknown_codes(cfg: &EffectiveConfig, known: &[&'static str]) -> Result<()> {
    let has_unknown = [&cfg.deny, &cfg.warn, &cfg.allow]
        .into_iter()
        .flatten()
        .any(|c| !is_wildcard(c) && !known.contains(&c.as_str()));
    if has_unknown {
        anyhow::bail!(
            "a --deny/--allow value is not a lint code. Pass a real code (e.g. `ACC001-P`), or `all`/`'*'` for every code (quote `*` so the shell does not expand it into filenames)."
        );
    }
    Ok(())
}

fn parse_confidence(s: Option<&str>) -> Confidence {
    match s.map(str::to_ascii_lowercase).as_deref() {
        Some("heuristic") => Confidence::Heuristic,
        Some("definite") => Confidence::Definite,
        _ => Confidence::Likely,
    }
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_rs_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}
