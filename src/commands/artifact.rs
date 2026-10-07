use crate::config::Arch;
use anyhow::Result;
use serde::{Deserialize, Serialize};
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

/// What `pinoc build` or `pinoc test` built, kept beside the artifact as
/// `<name>.build.json` so `pinoc test --no-build` can tell what it is testing.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct BuildRecord {
    pub features: Vec<String>,
    pub arch: Option<Arch>,
    /// Length and FNV-1a hash of the artifact, to tell when something other
    /// than pinoc has replaced it since.
    len: u64,
    hash: String,
}

/// Whether the artifact on disk is the one a `BuildRecord` describes.
pub enum Recorded {
    Yes(BuildRecord),
    /// No record, or the artifact has changed since it was written.
    No,
}

fn record_path(artifact: &Path) -> PathBuf {
    artifact.with_extension("build.json")
}

fn fingerprint(bytes: &[u8]) -> (u64, String) {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash = (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    (bytes.len() as u64, format!("{hash:016x}"))
}

fn normalized(features: &[String]) -> Vec<String> {
    let mut features = features.to_vec();
    features.sort();
    features.dedup();
    features
}

/// Records the features and arch the artifact was just built with.
pub fn record_build(manifest: Option<&toml::Table>, features: &[String], arch: Option<Arch>) {
    let Some(path) = manifest.and_then(artifact_path) else {
        return;
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return;
    };
    let (len, hash) = fingerprint(&bytes);
    let record = BuildRecord {
        features: normalized(features),
        arch,
        len,
        hash,
    };
    if let Ok(json) = serde_json::to_string_pretty(&record) {
        let _ = std::fs::write(record_path(&path), json + "\n");
    }
}

pub fn read_build_record(artifact: &Path) -> Recorded {
    let read = || {
        let record: BuildRecord =
            serde_json::from_str(&std::fs::read_to_string(record_path(artifact)).ok()?).ok()?;
        let (len, hash) = fingerprint(&std::fs::read(artifact).ok()?);
        (record.len == len && record.hash == hash).then_some(record)
    };
    read().map_or(Recorded::No, Recorded::Yes)
}

/// For `pinoc test --no-build`: errors if the artifact was built with other
/// features or another arch than this run would have built it with.
pub fn check_build_matches(
    manifest: Option<&toml::Table>,
    build_features: &[String],
    arch: Option<Arch>,
) -> Result<()> {
    let Some(path) = manifest.and_then(artifact_path) else {
        return Ok(());
    };
    if !path.exists() {
        return Ok(());
    }
    let record = match read_build_record(&path) {
        Recorded::Yes(record) => record,
        Recorded::No => {
            println!(
                "{} was not built by pinoc, or has been rebuilt since, so --no-build cannot tell which features and arch it has.",
                path.display()
            );
            return Ok(());
        }
    };
    let wanted = normalized(build_features);
    if record.features == wanted && record.arch == arch {
        return Ok(());
    }
    let describe = |features: &[String], arch: Option<Arch>| {
        let features = if features.is_empty() {
            "no features".to_string()
        } else {
            format!("--features {}", features.join(","))
        };
        match arch {
            Some(arch) => format!("{features} and --arch {}", arch.as_str()),
            None => features,
        }
    };
    let mut expect = format!("--build-features \"{}\"", record.features.join(","));
    if let Some(arch) = record.arch {
        expect.push_str(&format!(" --arch {}", arch.as_str()));
    }
    anyhow::bail!(
        "{} was built with {}, but this run would build it with {}. Drop --no-build to rebuild it, or pass {expect} if that is the artifact to test.",
        path.display(),
        describe(&record.features, record.arch),
        describe(&wanted, arch)
    )
}

/// The SBPF version an ELF64 little-endian program was built for, which the
/// toolchain records in `e_flags`. None if the bytes are not such a file.
pub fn sbpf_version(elf: &[u8]) -> Option<u32> {
    if elf.get(0..6)? != b"\x7fELF\x02\x01" {
        return None;
    }
    Some(u32::from_le_bytes(elf.get(0x30..0x34)?.try_into().ok()?))
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
