use crate::check::codes::{self, UNUSED_ALLOW};
use crate::check::contract::{Confidence, Finding, Severity};
use crate::check::locate::locate;
use anyhow::Result;
use std::io::IsTerminal;

fn confidence_name(c: Confidence) -> &'static str {
    match c {
        Confidence::Heuristic => "heuristic",
        Confidence::Likely => "likely",
        Confidence::Definite => "definite",
    }
}

fn color(code: &str, on: bool) -> &str {
    if on {
        code
    } else {
        ""
    }
}

pub fn render_human(findings: &[Finding], below_threshold: &[&str], threshold: Confidence) {
    let tty = std::io::stdout().is_terminal();
    let (red, yellow, blue, dim, bold, reset) = (
        color("\x1b[31m", tty),
        color("\x1b[33m", tty),
        color("\x1b[34m", tty),
        color("\x1b[2m", tty),
        color("\x1b[1m", tty),
        color("\x1b[0m", tty),
    );

    if findings.is_empty() {
        println!("✅ No issues found.");
        print_below_threshold(below_threshold, threshold, dim, reset);
        return;
    }

    let mut deny = 0;
    let mut warn = 0;
    for f in findings {
        let (badge, tint) = match f.severity {
            Severity::Deny => {
                deny += 1;
                ("deny", red)
            }
            Severity::Warn => {
                warn += 1;
                ("warn", yellow)
            }
            Severity::Info => ("info", blue),
        };
        println!(
            "{tint}{bold}{badge}{reset} {tint}{}{reset} {dim}{}:{}:{}{reset}",
            f.code, f.span.file, f.span.line, f.span.col
        );
        println!("    {}", f.evidence);
        if let Some(fix) = &f.fix {
            println!("    {dim}fix:{reset} {fix}");
        }
    }
    println!("\n{deny} deny, {warn} warn");
    print_below_threshold(below_threshold, threshold, dim, reset);
}

/// Report how many findings were hidden only for being below the threshold,
/// and which codes, so `--deny <code>` can be used without guessing.
fn print_below_threshold(hidden: &[&str], threshold: Confidence, dim: &str, reset: &str) {
    if hidden.is_empty() {
        return;
    }
    let count = hidden.len();
    let plural = if count == 1 { "finding" } else { "findings" };
    let mut codes: Vec<&str> = hidden.to_vec();
    codes.sort_unstable();
    codes.dedup();
    println!(
        "{dim}{count} lower-confidence {plural} ({}) below the `{}` threshold hidden; lower `confidence_threshold` (or `--deny <code>`) to show them.{reset}",
        codes.join(", "),
        confidence_name(threshold)
    );
}

pub fn render_json(findings: &[Finding]) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(findings)?);
    Ok(())
}

/// SARIF 2.1.0 for code scanning. Every code is a rule on every run, so a
/// result's `ruleIndex` means the same rule from one run to the next.
pub fn render_sarif(findings: &[Finding]) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let repo = env!("CARGO_PKG_REPOSITORY");
    let all = codes::all();
    let rules: Vec<serde_json::Value> = all
        .iter()
        .map(|c| {
            let tag = match c.category {
                Some(category) => serde_json::to_value(category).unwrap_or_default(),
                None if c.code == UNUSED_ALLOW => "meta".into(),
                None => "coverage".into(),
            };
            let mut tags = vec![tag];
            if c.category.is_some() {
                tags.push("security".into());
            }
            serde_json::json!({
                "id": c.code,
                "name": c.id,
                "shortDescription": { "text": c.summary },
                "fullDescription": { "text": c.explain.split("\n\n").nth(2).unwrap_or(c.summary) },
                "help": { "text": c.explain, "markdown": c.explain },
                "helpUri": format!("{repo}/blob/v{version}/docs/lints/{}.md", c.code),
                "defaultConfiguration": { "level": sarif_level(c.default_severity) },
                "properties": {
                    "tags": tags,
                    "precision": sarif_precision(c.default_confidence),
                },
            })
        })
        .collect();
    let results: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            let text = message_with_fix(f);
            let mut result = serde_json::json!({
                "ruleId": f.code,
                "ruleIndex": all.iter().position(|c| c.code == f.code),
                "level": sarif_level(f.severity),
                "message": { "text": text },
            });
            if let Some(at) = locate(&f.span) {
                result["locations"] = serde_json::json!([{
                    "physicalLocation": {
                        "artifactLocation": { "uri": at.file },
                        "region": { "startLine": at.line, "startColumn": at.col },
                    }
                }]);
            }
            result
        })
        .collect();
    let sarif = serde_json::json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "pinoc",
                    "version": version,
                    "informationUri": repo,
                    "rules": rules,
                }
            },
            "results": results,
        }],
    });
    println!("{}", serde_json::to_string_pretty(&sarif)?);
    Ok(())
}

fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Deny => "error",
        Severity::Warn => "warning",
        Severity::Info => "note",
    }
}

fn sarif_precision(confidence: Confidence) -> &'static str {
    match confidence {
        Confidence::Definite => "very-high",
        Confidence::Likely => "high",
        Confidence::Heuristic => "medium",
    }
}

/// GitHub Actions workflow commands: one annotation per finding, placed on the
/// file and line from the repository root, then the summary lines.
pub fn render_github(findings: &[Finding], below_threshold: &[&str], threshold: Confidence) {
    for f in findings {
        println!("{}", github_annotation(f));
    }
    let count = |sev| findings.iter().filter(|f| f.severity == sev).count();
    if findings.is_empty() {
        println!("✅ No issues found.");
    } else {
        println!(
            "{} deny, {} warn",
            count(Severity::Deny),
            count(Severity::Warn)
        );
    }
    print_below_threshold(below_threshold, threshold, "", "");
}

fn github_annotation(f: &Finding) -> String {
    let level = match f.severity {
        Severity::Deny => "error",
        Severity::Warn => "warning",
        Severity::Info => "notice",
    };
    let mut props = Vec::new();
    if let Some(at) = locate(&f.span) {
        props.push(format!("file={}", escape_property(&at.file)));
        props.push(format!("line={}", at.line));
        props.push(format!("col={}", at.col));
    }
    props.push(format!(
        "title={}",
        escape_property(&format!("{} {}", f.code, f.id))
    ));
    format!(
        "::{level} {}::{}",
        props.join(","),
        escape_data(&message_with_fix(f))
    )
}

/// The evidence, then the fix, as one sentence-separated message.
fn message_with_fix(f: &Finding) -> String {
    let mut message = f.evidence.clone();
    if let Some(fix) = &f.fix {
        let stop = if message.ends_with('.') { "" } else { "." };
        message.push_str(&format!("{stop} Fix: {fix}"));
    }
    message
}

/// Workflow-command escaping for the message.
fn escape_data(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

/// Workflow-command escaping for a property value, which also ends at `,` or `::`.
fn escape_property(s: &str) -> String {
    escape_data(s).replace(':', "%3A").replace(',', "%2C")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_command_text_is_escaped() {
        assert_eq!(escape_data("50% done\nnext\r"), "50%25 done%0Anext%0D");
        assert_eq!(escape_property("a::b, c"), "a%3A%3Ab%2C c");
    }
}
