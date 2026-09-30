use super::artifact::{
    check_built_artifact, enables_no_entrypoint, read_manifest, Rebuild, NO_ENTRYPOINT,
};
use super::build::{build_sbf, features_arg, split_features};
use anyhow::{Context, Result};
use std::io::Write;
use std::process::Command;

pub fn run_test(
    quiet: bool,
    features: &[String],
    build_features: Option<&[String]>,
    no_build: bool,
) -> Result<()> {
    // SVM tests load target/deploy/*.so, which `cargo test` does not rebuild.
    if !no_build {
        let manifest = read_manifest();
        let build_features = match build_features {
            Some(explicit) => split_features(explicit),
            None => without_no_entrypoint(&split_features(features), manifest.as_ref()),
        };
        build_sbf(quiet, &build_features)?;
        check_built_artifact(
            manifest.as_ref(),
            Rebuild::Test {
                test_features: &split_features(features),
                build_features: &build_features,
            },
        )?;
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

fn without_no_entrypoint(features: &[String], manifest: Option<&toml::Table>) -> Vec<String> {
    let Some(manifest) = manifest else {
        return features.to_vec();
    };
    let (dropped, kept): (Vec<String>, Vec<String>) = features
        .iter()
        .cloned()
        .partition(|f| enables_no_entrypoint(f, manifest));
    if !dropped.is_empty() {
        let names = dropped
            .iter()
            .map(|f| format!("`{f}`"))
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "Leaving {names} out of the SBF build: it enables `{NO_ENTRYPOINT}`, which builds an artifact with no program in it. Tests still run with it. Pass --build-features to set the build features explicitly."
        );
    }
    kept
}
