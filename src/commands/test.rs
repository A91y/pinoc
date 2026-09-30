use super::build::{build_sbf, features_arg, split_features};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const NO_ENTRYPOINT: &str = "no-entrypoint";

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
        check_artifact(
            manifest.as_ref(),
            &split_features(features),
            &build_features,
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

fn read_manifest() -> Option<toml::Table> {
    let content = std::fs::read_to_string("Cargo.toml").ok()?;
    toml::from_str(&content).ok()
}

fn package_name(manifest: &toml::Table) -> Option<&str> {
    manifest.get("package")?.get("name")?.as_str()
}

/// Resolves `pkg/feat` for this package to `feat`; returns None for a dependency's feature.
fn local_feature<'a>(feature: &'a str, manifest: &toml::Table) -> Option<&'a str> {
    match feature.split_once('/') {
        None => Some(feature),
        Some((pkg, feat)) if Some(pkg) == package_name(manifest) => Some(feat),
        Some(_) => None,
    }
}

/// Whether `feature` is or transitively enables this package's `no-entrypoint` feature.
fn enables_no_entrypoint(feature: &str, manifest: &toml::Table) -> bool {
    let table = manifest.get("features").and_then(|f| f.as_table());
    let mut stack: Vec<&str> = local_feature(feature, manifest).into_iter().collect();
    let mut seen = HashSet::new();
    while let Some(f) = stack.pop() {
        if f == NO_ENTRYPOINT {
            return true;
        }
        if !seen.insert(f) {
            continue;
        }
        let members = table.and_then(|t| t.get(f)).and_then(|m| m.as_array());
        for m in members.into_iter().flatten().filter_map(|m| m.as_str()) {
            // `dep:x` and `x/y` / `x?/y` point at dependencies, not at this package's features.
            if !m.starts_with("dep:") && !m.contains('/') {
                stack.push(m);
            }
        }
    }
    false
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

fn artifact_path(manifest: &toml::Table) -> Option<PathBuf> {
    let lib_name = manifest
        .get("lib")
        .and_then(|l| l.get("name"))
        .and_then(|n| n.as_str());
    let name = lib_name
        .or_else(|| package_name(manifest))?
        .replace('-', "_");
    Some(Path::new("target/deploy").join(format!("{name}.so")))
}

fn check_artifact(
    manifest: Option<&toml::Table>,
    test_features: &[String],
    build_features: &[String],
) -> Result<()> {
    let Some(manifest) = manifest else {
        return Ok(());
    };
    let Some(path) = artifact_path(manifest) else {
        return Ok(());
    };
    let Ok(elf) = std::fs::read(&path) else {
        return Ok(());
    };
    if has_entrypoint(&elf) != Some(false) {
        return Ok(());
    }

    let mut msg = format!(
        "Built artifact {} ({} bytes) has no `entrypoint` symbol, so it contains no program to load.",
        path.display(),
        elf.len()
    );
    let culprits: Vec<&String> = build_features
        .iter()
        .filter(|f| enables_no_entrypoint(f, manifest))
        .collect();
    if !culprits.is_empty() {
        let names = culprits
            .iter()
            .map(|f| format!("`{f}`"))
            .collect::<Vec<_>>()
            .join(", ");
        let kept = build_features
            .iter()
            .filter(|f| !enables_no_entrypoint(f, manifest))
            .cloned()
            .collect::<Vec<_>>()
            .join(",");
        let test_flag = if test_features.is_empty() {
            String::new()
        } else {
            format!("--features {} ", test_features.join(","))
        };
        msg.push_str(&format!(
            "\nThe build features {names} enable `{NO_ENTRYPOINT}`. Build the artifact without them and pass the test features separately:\n\n    pinoc test {test_flag}--build-features \"{kept}\""
        ));
    } else if enables_no_entrypoint("default", manifest) {
        msg.push_str(&format!(
            "\nThe `default` feature enables `{NO_ENTRYPOINT}`. Remove it from `default` so the deployable build keeps its entrypoint."
        ));
    } else {
        msg.push_str(
            "\nThe program does not declare an entrypoint. Add one, e.g. `pinocchio::entrypoint!(process_instruction);`.",
        );
    }
    anyhow::bail!(msg)
}

/// Whether an ELF64 little-endian binary defines an `entrypoint` symbol.
/// None if the bytes are not a parseable ELF64 LE file.
fn has_entrypoint(elf: &[u8]) -> Option<bool> {
    let u16_at = |o: usize| Some(u16::from_le_bytes(elf.get(o..o + 2)?.try_into().ok()?));
    let u32_at = |o: usize| Some(u32::from_le_bytes(elf.get(o..o + 4)?.try_into().ok()?));
    let u64_at = |o: usize| Some(u64::from_le_bytes(elf.get(o..o + 8)?.try_into().ok()?) as usize);

    if elf.get(0..6)? != b"\x7fELF\x02\x01" {
        return None;
    }
    let shoff = u64_at(0x28)?;
    let shentsize = u16_at(0x3a)? as usize;
    let shnum = u16_at(0x3c)? as usize;
    let section = |i: usize| shoff + i * shentsize;

    for i in 0..shnum {
        let sh = section(i);
        let sh_type = u32_at(sh + 4)?;
        // SHT_SYMTAB = 2, SHT_DYNSYM = 11
        if sh_type != 2 && sh_type != 11 {
            continue;
        }
        let (offset, size) = (u64_at(sh + 0x18)?, u64_at(sh + 0x20)?);
        let entsize = u64_at(sh + 0x38)?;
        let strtab = u64_at(section(u32_at(sh + 0x28)? as usize) + 0x18)?;
        if entsize == 0 {
            continue;
        }
        for sym in (offset..offset + size).step_by(entsize) {
            let name_start = strtab + u32_at(sym)? as usize;
            let shndx = u16_at(sym + 6)?;
            let name = elf.get(name_start..)?.split(|&b| b == 0).next()?;
            if name == b"entrypoint" && shndx != 0 {
                return Some(true);
            }
        }
    }
    Some(false)
}
