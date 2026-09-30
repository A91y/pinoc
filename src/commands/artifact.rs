use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const NO_ENTRYPOINT: &str = "no-entrypoint";

/// Which command produced (or is about to use) the artifact, so the error can suggest the
/// right way to rebuild it.
pub enum Rebuild<'a> {
    Test {
        test_features: &'a [String],
        build_features: &'a [String],
    },
    Build {
        build_features: &'a [String],
    },
    Deploy,
}

pub fn read_manifest() -> Option<toml::Table> {
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
pub fn enables_no_entrypoint(feature: &str, manifest: &toml::Table) -> bool {
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

pub fn artifact_path(manifest: &toml::Table) -> Option<PathBuf> {
    let lib_name = manifest
        .get("lib")
        .and_then(|l| l.get("name"))
        .and_then(|n| n.as_str());
    let name = lib_name
        .or_else(|| package_name(manifest))?
        .replace('-', "_");
    Some(Path::new("target/deploy").join(format!("{name}.so")))
}

/// Checks the artifact `cargo build-sbf` just produced for this package, deleting it if it
/// has no entrypoint so a later `pinoc deploy` or `pinoc test --no-build` cannot pick it up.
pub fn check_built_artifact(manifest: Option<&toml::Table>, rebuild: Rebuild) -> Result<()> {
    let Some(path) = manifest.and_then(artifact_path) else {
        return Ok(());
    };
    if let Err(e) = check_entrypoint(&path, manifest, rebuild) {
        if std::fs::remove_file(&path).is_ok() {
            anyhow::bail!(
                "{e}\nRemoved {} so it cannot be deployed or tested.",
                path.display()
            );
        }
        return Err(e);
    }
    Ok(())
}

/// Errors if the SBF artifact at `path` has no entrypoint, naming the likely cause.
/// A missing or non-ELF file passes; the caller's own error handling covers it.
pub fn check_entrypoint(
    path: &Path,
    manifest: Option<&toml::Table>,
    rebuild: Rebuild,
) -> Result<()> {
    let Ok(elf) = std::fs::read(path) else {
        return Ok(());
    };
    if has_entrypoint(&elf) != Some(false) {
        return Ok(());
    }

    let mut msg = format!(
        "Artifact {} ({} bytes) has no entrypoint (its ELF entry address is outside the program code), so it contains no program to load.",
        path.display(),
        elf.len()
    );
    let enables = |f: &String| manifest.is_some_and(|m| enables_no_entrypoint(f, m));
    let quoted = |fs: &[&String]| {
        fs.iter()
            .map(|f| format!("`{f}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let build_features = match &rebuild {
        Rebuild::Test { build_features, .. } | Rebuild::Build { build_features } => {
            Some(*build_features)
        }
        Rebuild::Deploy => None,
    };
    let culprits: Vec<&String> = build_features
        .unwrap_or_default()
        .iter()
        .filter(|f| enables(f))
        .collect();
    let kept = build_features
        .unwrap_or_default()
        .iter()
        .filter(|f| !enables(f))
        .cloned()
        .collect::<Vec<_>>()
        .join(",");

    if !culprits.is_empty() {
        msg.push_str(&format!(
            "\nThe build features {} enable `{NO_ENTRYPOINT}`, which compiles the program out of the SBF build.",
            quoted(&culprits)
        ));
        match rebuild {
            Rebuild::Test { test_features, .. } => {
                let test_flag = if test_features.is_empty() {
                    String::new()
                } else {
                    format!("--features {} ", test_features.join(","))
                };
                msg.push_str(&format!(
                    " Build the artifact without them and pass the test features separately:\n\n    pinoc test {test_flag}--build-features \"{kept}\""
                ));
            }
            _ => {
                let cmd = if kept.is_empty() {
                    "pinoc build".to_string()
                } else {
                    format!("pinoc build --features {kept}")
                };
                msg.push_str(&format!(" Build without them:\n\n    {cmd}"));
            }
        }
    } else if manifest.is_some_and(|m| enables_no_entrypoint("default", m)) {
        msg.push_str(&format!(
            "\nThe `default` feature enables `{NO_ENTRYPOINT}`. Remove it from `default` so the deployable build keeps its entrypoint."
        ));
    } else if matches!(rebuild, Rebuild::Deploy) {
        let mut suspects: Vec<String> = manifest
            .and_then(|m| m.get("features")?.as_table().map(|t| (m, t)))
            .map(|(m, t)| {
                t.keys()
                    .filter(|f| enables_no_entrypoint(f, m))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        suspects.sort();
        if suspects.is_empty() {
            msg.push_str("\nThe program may not declare an entrypoint (e.g. `pinocchio::entrypoint!(process_instruction);`). Rebuild with `pinoc build` before deploying.");
        } else {
            msg.push_str(&format!(
                "\nIt was likely built with a feature that enables `{NO_ENTRYPOINT}` ({}). Rebuild without it before deploying:\n\n    pinoc build",
                quoted(&suspects.iter().collect::<Vec<_>>())
            ));
        }
    } else {
        msg.push_str(
            "\nThe program does not declare an entrypoint. Add one, e.g. `pinocchio::entrypoint!(process_instruction);`.",
        );
    }
    anyhow::bail!(msg)
}

/// Whether an ELF64 little-endian binary's entry address (`e_entry`) falls inside an executable
/// section, which is what the SBF loader requires. None if the bytes are not a parseable ELF64 LE file.
fn has_entrypoint(elf: &[u8]) -> Option<bool> {
    let u16_at = |o: usize| Some(u16::from_le_bytes(elf.get(o..o + 2)?.try_into().ok()?));
    let u64_at = |o: usize| Some(u64::from_le_bytes(elf.get(o..o + 8)?.try_into().ok()?));

    if elf.get(0..6)? != b"\x7fELF\x02\x01" {
        return None;
    }
    let entry = u64_at(0x18)?;
    let shoff = u64_at(0x28)? as usize;
    let shentsize = u16_at(0x3a)? as usize;
    let shnum = u16_at(0x3c)? as usize;

    const SHF_EXECINSTR: u64 = 0x4;
    for i in 0..shnum {
        let sh = shoff + i * shentsize;
        if u64_at(sh + 0x08)? & SHF_EXECINSTR == 0 {
            continue;
        }
        let (addr, size) = (u64_at(sh + 0x10)?, u64_at(sh + 0x20)?);
        if (addr..addr + size).contains(&entry) {
            return Some(true);
        }
    }
    Some(false)
}
