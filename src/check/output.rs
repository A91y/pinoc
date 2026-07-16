use crate::check::contract::{Finding, Severity};
use anyhow::Result;
use std::io::IsTerminal;

fn color(code: &str, on: bool) -> &str {
    if on {
        code
    } else {
        ""
    }
}

pub fn render_human(findings: &[Finding]) {
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
}

pub fn render_json(findings: &[Finding]) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(findings)?);
    Ok(())
}
