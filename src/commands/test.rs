use super::build::{build_sbf, features_arg};
use anyhow::{Context, Result};
use std::io::Write;
use std::process::Command;

pub fn run_test(quiet: bool, features: &[String], no_build: bool) -> Result<()> {
    // SVM tests load target/deploy/*.so, which `cargo test` does not rebuild.
    if !no_build {
        build_sbf(quiet, features)?;
    }

    println!("Testing program");
    let mut cmd = Command::new("cargo");
    cmd.arg("test");
    if let Some(features) = features_arg(features) {
        cmd.arg("--features").arg(features);
    }
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
