use crate::idl::{generate_idl, Generator};
use anyhow::{Context, Result};
use std::process::Command;

pub fn run_build(
    quiet: bool,
    program_id: Option<&str>,
    idl_generator: Option<Generator>,
) -> Result<()> {
    println!("Building program");
    let mut cmd = Command::new("cargo");
    cmd.arg("build-sbf");
    if quiet {
        cmd.arg("--").arg("--quiet");
    }

    let status = cmd.spawn()?.wait().context("Failed to build project")?;
    if !status.success() {
        anyhow::bail!("Build failed with exit code: {:?}", status.code());
    } else {
        println!("Build completed successfully!");
    }

    if let Err(e) = generate_idl("target/idl", program_id, idl_generator) {
        let full_message = e
            .chain()
            .map(|cause| cause.to_string())
            .collect::<Vec<_>>()
            .join(": ");
        println!("⚠️  Skipped IDL generation: {full_message}");
        if full_message.contains("declare_id") {
            println!(
                "   If your program doesn't use `declare_id!`, pass `--program-id <ADDRESS>` to `pinoc build`."
            );
        } else if full_message.contains("[idl].generator") {
            println!("   Fix the `[idl].generator` value in Pinoc.toml, or override it with `--idl-generator <shank|codama>`.");
        }
    }

    Ok(())
}
