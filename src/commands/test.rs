use super::artifact::{
    check_build_matches, check_built_artifact, enables_no_entrypoint, read_manifest, record_build,
    Rebuild, NO_ENTRYPOINT,
};
use super::build::{
    build_sbf, build_sbf_in, cluster_features, features_arg, merge_features, resolve_arch,
    split_features,
};
use crate::config::{self, Arch};
use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;
use std::process::Command;

pub struct TestArgs<'a> {
    pub quiet: bool,
    pub features: &'a [String],
    pub build_features: Option<&'a [String]>,
    pub cluster: Option<&'a str>,
    pub all_clusters: bool,
    pub arch: Option<Arch>,
    pub no_build: bool,
    pub cargo_args: &'a [String],
}

pub fn run_test(args: TestArgs) -> Result<()> {
    if !args.all_clusters {
        return run_once(&args, args.cluster);
    }
    let clusters = config::load()?.map(|c| c.clusters).unwrap_or_default();
    if clusters.is_empty() {
        anyhow::bail!(
            "--all-clusters runs the tests once per `[clusters.<name>]` in Pinoc.toml, and it declares none."
        );
    }
    // Clusters built with the same features share an artifact, so they share a run.
    let mut runs: Vec<(Vec<String>, Vec<&String>)> = Vec::new();
    for (name, cluster) in &clusters {
        let mut features = split_features(&cluster.features);
        features.sort();
        features.dedup();
        match runs.iter_mut().find(|(f, _)| *f == features) {
            Some((_, names)) => names.push(name),
            None => runs.push((features, vec![name])),
        }
    }
    let mut failed = Vec::new();
    for (_, names) in &runs {
        let label = names
            .iter()
            .map(|n| format!("`{n}`"))
            .collect::<Vec<_>>()
            .join(", ");
        println!("\n== Testing cluster {label}");
        if let Err(e) = run_once(&args, Some(names[0])) {
            eprintln!("Error: {e:#}");
            failed.push(label);
        }
    }
    if !failed.is_empty() {
        anyhow::bail!(
            "Tests failed for cluster {} ({} of {} runs).",
            failed.join(", "),
            failed.len(),
            runs.len()
        );
    }
    println!("\nTested all {} cluster runs successfully!", runs.len());
    Ok(())
}

fn run_once(args: &TestArgs, cluster: Option<&str>) -> Result<()> {
    let TestArgs {
        quiet,
        features,
        build_features,
        arch,
        no_build,
        cargo_args,
        ..
    } = *args;
    let arch = resolve_arch(arch)?;
    let manifest = read_manifest();
    let test_config = config::load()?.map(|c| c.test).unwrap_or_default();
    let cluster_features = cluster_features(cluster)?;
    // `[test] features`, then the cluster's, then the flag's.
    let features = merge_features(
        &merge_features(&test_config.features, &cluster_features),
        features,
    );
    let build_features = match build_features {
        Some(explicit) => merge_features(&cluster_features, explicit),
        None => without_no_entrypoint(&features, manifest.as_ref(), !no_build),
    };
    // SVM tests load target/deploy/*.so, which `cargo test` does not rebuild.
    if no_build {
        check_build_matches(manifest.as_ref(), &build_features, arch)?;
    } else {
        build_sbf(quiet, &build_features, arch)?;
        check_built_artifact(
            manifest.as_ref(),
            Rebuild::Test {
                test_features: &features,
                build_features: &build_features,
            },
        )?;
        record_build(manifest.as_ref(), &build_features, arch, cluster);
        build_test_programs(&test_config.programs, quiet, arch)?;
    }

    println!("Testing program");
    let mut cmd = Command::new("cargo");
    cmd.arg("test");
    if let Some(features) = features_arg(&features) {
        cmd.arg("--features").arg(features);
    }
    cmd.args(cargo_args);
    if quiet {
        if !cargo_args.iter().any(|a| a == "--") {
            cmd.arg("--");
        }
        cmd.arg("--quiet");
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

/// `[test] programs`: other SBF programs the tests load, each built in its own
/// directory with the arch the program under test was built with.
fn build_test_programs(programs: &[String], quiet: bool, arch: Option<Arch>) -> Result<()> {
    for program in programs {
        let dir = Path::new(program);
        if !dir.join("Cargo.toml").exists() {
            anyhow::bail!(
                "`[test] programs` lists {program:?}, which has no Cargo.toml. Each entry is the directory of a program the tests load."
            );
        }
        println!("Building test program {program}");
        build_sbf_in(Some(dir), quiet, &[], arch)?;
    }
    Ok(())
}

fn without_no_entrypoint(
    features: &[String],
    manifest: Option<&toml::Table>,
    announce: bool,
) -> Vec<String> {
    let Some(manifest) = manifest else {
        return features.to_vec();
    };
    let (dropped, kept): (Vec<String>, Vec<String>) = features
        .iter()
        .cloned()
        .partition(|f| enables_no_entrypoint(f, manifest));
    if announce && !dropped.is_empty() {
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
