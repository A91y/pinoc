pub mod contract;
pub mod facts;
pub mod lints;
pub mod output;
pub mod suppress;

use crate::check::contract::{Confidence, Finding, ParsedFile, Severity, Span};
use crate::check::suppress::Suppressions;
use crate::config;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Coverage findings: they report what the run could not analyse, not a defect in
/// the program. Both are configurable like any lint code.
const NO_HANDLERS: &str = "NO-HANDLERS";
const UNTRACKED_ACCOUNTS: &str = "UNTRACKED-ACCOUNTS";

pub struct CheckOptions {
    pub json: bool,
    pub deny: Vec<String>,
    pub allow: Vec<String>,
}

pub(crate) struct EffectiveConfig {
    // From `Pinoc.toml [check]`.
    pub deny: Vec<String>,
    pub warn: Vec<String>,
    pub allow: Vec<String>,
    // From `--deny`/`--allow`; these override the config file per code.
    pub cli_deny: Vec<String>,
    pub cli_allow: Vec<String>,
    pub threshold: Confidence,
    pub authority_names: Vec<String>,
}

/// Effective disposition of one lint code.
enum Disposition {
    Allow,
    Deny,
    Warn,
    Default,
}

/// CLI flags override the config file; within a layer `allow` beats `deny`. A
/// `*`/`all` wildcard only matches in its own layer, so a specific `--deny X`
/// still overrides a config `allow = ["*"]`.
fn resolve(cfg: &EffectiveConfig, code: &str) -> Disposition {
    if code_matches(&cfg.cli_allow, code) {
        Disposition::Allow
    } else if code_matches(&cfg.cli_deny, code) {
        Disposition::Deny
    } else if code_matches(&cfg.allow, code) {
        Disposition::Allow
    } else if code_matches(&cfg.deny, code) {
        Disposition::Deny
    } else if code_matches(&cfg.warn, code) {
        Disposition::Warn
    } else {
        Disposition::Default
    }
}

pub fn run(opts: CheckOptions) -> Result<i32> {
    let cfg = load_effective_config(&opts)?;

    let lints = lints::registry();
    let mut known: Vec<&'static str> = lints.iter().map(|l| l.code()).collect();
    known.extend([NO_HANDLERS, UNTRACKED_ACCOUNTS]);
    reject_unknown_codes(&cfg, &known)?;

    let src_dir = Path::new("src");
    let mut files = Vec::new();
    if src_dir.exists() {
        collect_rs_files(src_dir, &mut files)?;
    }

    let mut supp = Suppressions::default();
    let mut parsed = Vec::new();
    for path in &files {
        let src = std::fs::read_to_string(path)?;
        supp.scan(&path.display().to_string(), &src);
        let Ok(ast) = syn::parse_file(&src) else {
            continue;
        };
        parsed.push(ParsedFile {
            path: path.clone(),
            src,
            ast,
        });
    }

    // The account and CPI lints run on fact tables built over the whole crate.
    let handlers = facts::analyze(&parsed, &cfg.authority_names);
    // Instructions that share an accounts type are analysed separately, and
    // report the same finding once.
    let mut by_lint: Vec<Vec<Finding>> = lints
        .iter()
        .map(|l| {
            let mut seen = std::collections::HashSet::new();
            l.run_handlers(&handlers)
                .into_iter()
                .filter(|f| seen.insert((f.span.file.clone(), f.span.line, f.span.col)))
                .collect()
        })
        .collect();

    // Findings are emitted file by file, each lint in turn within a file.
    let mut raw = Vec::new();
    for file in &parsed {
        let name = file.path.display().to_string();
        for (lint, crate_wide) in lints.iter().zip(&mut by_lint) {
            raw.extend(lint.run(file));
            let (here, rest) = std::mem::take(crate_wide)
                .into_iter()
                .partition(|f: &Finding| f.span.file == name);
            raw.extend::<Vec<Finding>>(here);
            *crate_wide = rest;
        }
    }

    // Handlers with accounts that were not analysed: none could be bound from
    // the slice, or they were stored in a type that nothing reads them back from.
    let mut storing: Vec<(String, Span)> = Vec::new();
    let mut listed = std::collections::HashSet::new();
    for h in &handlers {
        let entry = if h.unbound_accounts {
            let span = Span {
                file: h.file.clone(),
                line: 0,
                col: 0,
            };
            Some((
                format!("`{}` (binds no account from its slice)", h.name),
                span,
            ))
        } else {
            h.stores_accounts
                .as_ref()
                .filter(|_| h.followed_by.is_empty())
                .map(|site| {
                    (
                        format!("`{}` (stores them where nothing reads them back)", h.name),
                        site.to_span(),
                    )
                })
        };
        if let Some(entry) = entry {
            if listed.insert((h.file.clone(), h.name.clone())) {
                storing.push(entry);
            }
        }
    }
    storing.sort_by(|a, b| (&a.1.file, a.1.line).cmp(&(&b.1.file, b.1.line)));
    raw.extend(coverage_findings(
        src_dir.exists(),
        files.len(),
        handlers.len(),
        &storing,
    ));

    let processed = process_findings(raw, &cfg, &mut supp);
    if opts.json {
        output::render_json(&processed.findings)?;
    } else {
        output::render_human(
            &processed.findings,
            &processed.below_threshold,
            cfg.threshold,
        );
    }
    Ok(processed.exit_code)
}

/// Findings for the parts of the program the run could not analyse, so an empty
/// result is never mistaken for a clean one.
fn coverage_findings(
    src_found: bool,
    file_count: usize,
    handler_count: usize,
    storing: &[(String, Span)],
) -> Vec<Finding> {
    let mut out = Vec::new();
    if handler_count == 0 {
        let (evidence, fix) = if src_found {
            (
                format!(
                    "no instruction handlers found in {file_count} source file(s): no function or method takes an `&[AccountView]`/`&[AccountInfo]` slice, so the account and CPI lints (ACC*, CPI*, ZC002-P) analysed nothing. Only the struct-layout lints (ZC001-P, ZC003-P) ran. This is not a clean result"
                ),
                "if this crate is not a program, allow `NO-HANDLERS`; otherwise its handlers take their accounts in a form `pinoc check` does not follow",
            )
        } else {
            (
                "no `src/` directory in the current directory, so nothing was analysed. This is not a clean result".to_string(),
                "run `pinoc check` from the program crate's root",
            )
        };
        out.push(Finding {
            code: NO_HANDLERS,
            id: "no-handlers",
            confidence: Confidence::Definite,
            severity: Severity::Warn,
            span: Span {
                file: "src".to_string(),
                line: 0,
                col: 0,
            },
            evidence,
            fix: Some(fix.to_string()),
        });
    }
    if !storing.is_empty() {
        const SHOWN: usize = 5;
        let mut names = storing
            .iter()
            .take(SHOWN)
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>()
            .join(", ");
        if storing.len() > SHOWN {
            names.push_str(&format!(", and {} more", storing.len() - SHOWN));
        }
        out.push(Finding {
            code: UNTRACKED_ACCOUNTS,
            id: "untracked-accounts",
            confidence: Confidence::Definite,
            severity: Severity::Warn,
            // A project-wide limit, so it is not pinned to any one of the handlers.
            span: Span {
                file: "src".to_string(),
                line: 0,
                col: 0,
            },
            evidence: format!(
                "{} handler(s) have accounts that were not analysed: {names}. Checks and uses pinoc could not attach to an account are not reported, so this is not a clean result for them",
                storing.len()
            ),
            fix: Some(
                "no code change needed; allow `UNTRACKED-ACCOUNTS` once the limit is acknowledged"
                    .to_string(),
            ),
        });
    }
    out
}

/// Outcome of applying config and suppression to the raw findings.
pub(crate) struct Processed {
    pub findings: Vec<Finding>,
    pub exit_code: i32,
    /// The code of each finding dropped only because its confidence is below
    /// the threshold.
    pub below_threshold: Vec<&'static str>,
}

/// Applies config severity, inline suppression, and the confidence threshold to
/// raw findings; appends unused-allow findings; returns the survivors, the exit
/// code (nonzero iff any survivor is `Deny`), and how many were hidden by the
/// threshold.
pub(crate) fn process_findings(
    raw: Vec<Finding>,
    cfg: &EffectiveConfig,
    supp: &mut Suppressions,
) -> Processed {
    let mut out = Vec::new();
    let mut below_threshold = Vec::new();
    for mut f in raw {
        let denied = match resolve(cfg, f.code) {
            Disposition::Allow => continue,
            Disposition::Deny => {
                f.severity = Severity::Deny;
                true
            }
            Disposition::Warn => {
                f.severity = Severity::Warn;
                false
            }
            Disposition::Default => false,
        };
        if supp.is_suppressed(&f.span.file, f.span.line, f.code) {
            continue;
        }
        // A weak finding survives the threshold only when explicitly denied.
        if f.confidence < cfg.threshold && !denied {
            below_threshold.push(f.code);
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
    Processed {
        findings: out,
        exit_code,
        below_threshold,
    }
}

fn load_effective_config(opts: &CheckOptions) -> Result<EffectiveConfig> {
    let check = config::read_pinoc_config_optional()?
        .map(|c| c.check)
        .unwrap_or_default();
    Ok(EffectiveConfig {
        deny: check.deny,
        warn: check.warn,
        allow: check.allow,
        cli_deny: opts.deny.clone(),
        cli_allow: opts.allow.clone(),
        threshold: parse_confidence(check.confidence_threshold.as_deref())?,
        authority_names: check.authority_names,
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
    let lists = [
        (&cfg.deny, "`deny` under `[check]` in Pinoc.toml"),
        (&cfg.warn, "`warn` under `[check]` in Pinoc.toml"),
        (&cfg.allow, "`allow` under `[check]` in Pinoc.toml"),
        (&cfg.cli_deny, "`--deny`"),
        (&cfg.cli_allow, "`--allow`"),
    ];
    for (list, from) in lists {
        for code in list {
            if !is_wildcard(code) && !known.contains(&code.as_str()) {
                anyhow::bail!(
                    "`{code}`, given to {from}, is not a lint code. The codes are {}; `all` or `'*'` means every code (quote `*` so the shell does not expand it into filenames).",
                    known.join(", ")
                );
            }
        }
    }
    Ok(())
}

/// An unknown value is refused: falling back to the default would leave the
/// config saying one threshold while another is applied.
fn parse_confidence(s: Option<&str>) -> Result<Confidence> {
    match s.map(str::to_ascii_lowercase).as_deref() {
        None => Ok(Confidence::Likely),
        Some("heuristic") => Ok(Confidence::Heuristic),
        Some("likely") => Ok(Confidence::Likely),
        Some("definite") => Ok(Confidence::Definite),
        Some(_) => anyhow::bail!(
            "`confidence_threshold = \"{}\"` under `[check]` in Pinoc.toml is not a confidence level; use `heuristic`, `likely` or `definite`",
            s.unwrap_or_default()
        ),
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
