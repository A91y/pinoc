use std::process::Command;

use anyhow::Context;

pub fn build() -> Result<(), anyhow::Error> {
    println!("Building program");
    let status = Command::new("cargo")
        .arg("build-sbf")
        .spawn()?
        .wait()
        .with_context(|| "Failed to build project")?;

    if !status.success() {
        anyhow::bail!("Build failed with exit code: {:?}", status.code());
    } else {
        println!("Build completed successfully!");
        Ok(())
    }
}
