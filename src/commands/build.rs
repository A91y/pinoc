use crate::idl::{generate_idl, Generator};
use anyhow::{Context, Result};
use std::process::Command;

/// Splits repeated and comma/space-separated `--features` values into individual features,
/// the way cargo reads them.
pub fn split_features(features: &[String]) -> Vec<String> {
    features
        .iter()
        .flat_map(|f| f.split(|c: char| c == ',' || c.is_whitespace()))
        .filter(|f| !f.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn features_arg(features: &[String]) -> Option<String> {
    let list = split_features(features);
    (!list.is_empty()).then(|| list.join(","))
}

pub fn build_sbf(quiet: bool, features: &[String]) -> Result<()> {
    println!("Building program");
    let mut cmd = Command::new("cargo");
    cmd.arg("build-sbf");
    if let Some(features) = features_arg(features) {
        cmd.arg("--features").arg(features);
    }
    if quiet {
        cmd.arg("--").arg("--quiet");
    }

    let status = cmd.spawn()?.wait().context("Failed to build project")?;
    if !status.success() {
        anyhow::bail!("Build failed with exit code: {:?}", status.code());
    }
    println!("Build completed successfully!");
    Ok(())
}

pub fn run_build(
    quiet: bool,
    features: &[String],
    program_id: Option<&str>,
    idl_generator: Option<Generator>,
) -> Result<()> {
    build_sbf(quiet, features)?;

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
