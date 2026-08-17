use crate::check::contract::{Confidence, Finding, Severity};
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

pub fn render_human(findings: &[Finding], below_threshold: usize, threshold: Confidence) {
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

/// Report how many findings were hidden only for being below the threshold.
fn print_below_threshold(count: usize, threshold: Confidence, dim: &str, reset: &str) {
    if count == 0 {
        return;
    }
    let plural = if count == 1 { "finding" } else { "findings" };
    println!(
        "{dim}{count} lower-confidence {plural} below the `{}` threshold hidden; lower `confidence_threshold` (or `--deny <code>`) to show them.{reset}",
        confidence_name(threshold)
    );
}

pub fn render_json(findings: &[Finding]) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(findings)?);
    Ok(())
}
