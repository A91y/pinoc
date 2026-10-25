use super::artifact::{check_built_artifact, read_manifest, record_build, Rebuild};
use crate::config::{self, Arch};
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

/// `--arch` for `cargo build-sbf`: the flag, else `[build].arch` in `Pinoc.toml`.
/// With neither, the flag is not passed and the toolchain's default applies.
pub fn resolve_arch(arch: Option<Arch>) -> Result<Option<Arch>> {
    match arch {
        Some(arch) => Ok(Some(arch)),
        None => config::build_arch(),
    }
}

/// `[clusters.<name>].features`, the features an artifact for that cluster is built with.
pub fn cluster_features(cluster: Option<&str>) -> Result<Vec<String>> {
    match cluster {
        Some(name) => Ok(split_features(&config::cluster(name)?.1.features)),
        None => Ok(Vec::new()),
    }
}

/// `first`, then each of `rest` not already in it.
pub fn merge_features(first: &[String], rest: &[String]) -> Vec<String> {
    let mut out = split_features(first);
    for f in split_features(rest) {
        if !out.contains(&f) {
            out.push(f);
        }
    }
    out
}

pub fn build_sbf(quiet: bool, features: &[String], arch: Option<Arch>) -> Result<()> {
    println!("Building program");
    build_sbf_in(None, quiet, features, arch)
}

/// `cargo build-sbf`, in `dir` when given (another program's own workspace).
pub fn build_sbf_in(
    dir: Option<&std::path::Path>,
    quiet: bool,
    features: &[String],
    arch: Option<Arch>,
) -> Result<()> {
    let mut cmd = Command::new("cargo");
    if let Some(dir) = dir {
        cmd.current_dir(dir);
    }
    cmd.arg("build-sbf");
    if let Some(arch) = arch {
        cmd.arg("--arch").arg(arch.as_str());
    }
    if let Some(features) = features_arg(features) {
        cmd.arg("--features").arg(features);
    }
    if quiet {
        cmd.arg("--").arg("--quiet");
    }

    let status = cmd.spawn()?.wait().context("Failed to build project")?;
    if !status.success() {
        match dir {
            Some(dir) => anyhow::bail!(
                "Build of {} failed with exit code: {:?}",
                dir.display(),
                status.code()
            ),
            None => anyhow::bail!("Build failed with exit code: {:?}", status.code()),
        }
    }
    println!("Build completed successfully!");
    Ok(())
}

pub struct BuildArgs<'a> {
    pub quiet: bool,
    pub features: &'a [String],
    pub cluster: Option<&'a str>,
    pub arch: Option<Arch>,
    pub program_id: Option<&'a str>,
    pub idl_generator: Option<Generator>,
}

pub fn run_build(args: BuildArgs) -> Result<()> {
    let BuildArgs {
        quiet,
        features,
        cluster,
        arch,
        program_id,
        idl_generator,
    } = args;
    let arch = resolve_arch(arch)?;
    let manifest = read_manifest();
    let build_features = merge_features(&cluster_features(cluster)?, features);
    if let Some(name) = cluster {
        println!("Building for cluster `{name}`");
    }
    build_sbf(quiet, &build_features, arch)?;
    check_built_artifact(
        manifest.as_ref(),
        Rebuild::Build {
            build_features: &build_features,
        },
    )?;
    record_build(manifest.as_ref(), &build_features, arch, cluster);

    if let Err(e) = generate_idl(&config::idl_out_dir()?, program_id, idl_generator) {
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
