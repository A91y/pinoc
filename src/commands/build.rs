use crate::idl::generate_idl;
use anyhow::{Context, Result};
use std::process::Command;

pub fn run_build(quiet: bool, program_id: Option<&str>) -> Result<()> {
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

    if let Err(e) = generate_idl("target/idl", program_id) {
        let chain: Vec<String> = e.chain().map(|cause| cause.to_string()).collect();
        println!("⚠️  Skipped IDL generation: {}", chain.join(": "));
        println!(
            "   If your program doesn't use `declare_id!`, pass `--program-id <ADDRESS>` to `pinoc build`."
        );
    }

    Ok(())
}
