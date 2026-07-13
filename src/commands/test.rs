use anyhow::{Context, Result};
use std::io::Write;
use std::process::Command;

pub fn run_test(quiet: bool) -> Result<()> {
    println!("Testing program");
    let mut cmd = Command::new("cargo");
    cmd.arg("test");
    if quiet {
        cmd.arg("--").arg("--quiet");
    }

    let status = if quiet {
        // buffered so failures still print instead of just an exit code
        let output = cmd.output().context("Failed to test project")?;
        if !output.status.success() {
            std::io::stdout().write_all(&output.stdout)?;
            std::io::stderr().write_all(&output.stderr)?;
        }
        output.status
    } else {
        cmd.spawn()?.wait().context("Failed to test project")?
    };

    if !status.success() {
        anyhow::bail!("Test failed with exit code: {:?}", status.code());
    } else {
        println!("Tested successfully!");
    }

    Ok(())
}
