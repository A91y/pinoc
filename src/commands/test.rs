use std::process::Command;

use anyhow::Context;

pub fn test() -> Result<(), anyhow::Error> {
    println!("Testing program");
    let status = Command::new("cargo")
        .arg("test")
        .spawn()?
        .wait()
        .with_context(|| "Failed to test project")?;

    if !status.success() {
        anyhow::bail!("Test failed with exit code: {:?}", status.code());
    } else {
        println!("Tested successfully!");
        Ok(())
    }
}
