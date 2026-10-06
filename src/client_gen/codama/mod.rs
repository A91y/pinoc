//! Shells out to the real Codama JS pipeline to render a Rust or TypeScript
//! client. Node.js deps live in a project-local `<out_dir>/.pinoc-codama/`,
//! installed only with explicit consent (`--auto-install`), never silently.

use super::discriminants::{self, EnumDiscriminants};
use super::Language;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

/// The npm packages each renderer needs, at exact versions, and the lock file
/// for their whole dependency tree. A pinoc release therefore installs one
/// fixed set of packages, and a renderer upgrade is a change to pinoc: the enum
/// rewrite in `discriminants` depends on the renderers' exact output.
struct Tooling {
    package_json: &'static str,
    package_lock: &'static str,
}

const RUST_TOOLING: Tooling = Tooling {
    package_json: include_str!("npm/rust/package.json"),
    package_lock: include_str!("npm/rust/package-lock.json"),
};

const TS_TOOLING: Tooling = Tooling {
    package_json: include_str!("npm/ts/package.json"),
    package_lock: include_str!("npm/ts/package-lock.json"),
};

/// A copy of the lock file, written after `npm ci` succeeds. It records which
/// lock the installed `node_modules` came from.
const INSTALLED_LOCK: &str = "installed-lock.json";

/// A fingerprint of the installed `node_modules` contents, written with it.
/// `npm ci` checks the lock's integrity hashes only while installing; this is
/// what notices a tree changed afterwards (a restored cache, an edited file).
const INSTALLED_TREE: &str = "installed-tree";

impl Tooling {
    /// `(name, version)` of the packages named in `package.json`.
    fn packages(&self) -> Vec<(String, String)> {
        let manifest: serde_json::Value =
            serde_json::from_str(self.package_json).expect("embedded package.json is valid");
        manifest["dependencies"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(name, version)| {
                (
                    name.clone(),
                    version.as_str().unwrap_or_default().to_string(),
                )
            })
            .collect()
    }
}

const RUST_SCRIPT: &str = r#"import { rootNodeFromAnchor } from '@codama/nodes-from-anchor';
import { createFromRoot } from 'codama';
import { renderVisitor as renderRustVisitor } from '@codama/renderers-rust';
import fs from 'fs';

const [, , idlPath, generatedDir, crateFolder] = process.argv;
const idl = JSON.parse(fs.readFileSync(idlPath, 'utf-8'));
// A native Codama extraction (pinoc's codama_native module) is already a
// RootNode; only shank-shim output needs the Anchor-shaped conversion.
const rootNode = idl.kind === 'rootNode' ? idl : rootNodeFromAnchor(idl);
const codama = createFromRoot(rootNode);

await codama.accept(
  renderRustVisitor(generatedDir, {
    crateFolder,
    deleteFolderBeforeRendering: true,
    formatCode: false,
  }),
);

console.log('pinoc-codama: render complete');
"#;

// renderers-js takes the package folder and writes `src/generated` and a
// `package.json` (with the `@solana/kit` dependencies) inside it.
const TS_SCRIPT: &str = r#"import { rootNodeFromAnchor } from '@codama/nodes-from-anchor';
import { createFromRoot } from 'codama';
import { renderVisitor as renderJsVisitor } from '@codama/renderers-js';
import fs from 'fs';

const [, , idlPath, packageFolder] = process.argv;
const idl = JSON.parse(fs.readFileSync(idlPath, 'utf-8'));
const rootNode = idl.kind === 'rootNode' ? idl : rootNodeFromAnchor(idl);
const codama = createFromRoot(rootNode);

await codama.accept(
  renderJsVisitor(packageFolder, {
    deleteFolderBeforeRendering: true,
  }),
);

console.log('pinoc-codama: render complete');
"#;

/// Requires Node.js/npm.
pub fn generate_via_codama(
    idl_path: &Path,
    out_dir: &Path,
    auto_install: bool,
    language: Language,
    enums: &[EnumDiscriminants],
) -> Result<()> {
    check_node_available()?;

    let (tooling, script) = match language {
        Language::Rust => (&RUST_TOOLING, RUST_SCRIPT),
        Language::Ts => (&TS_TOOLING, TS_SCRIPT),
    };
    let tooling_dir = out_dir.join(".pinoc-codama");
    let node_modules = tooling_dir.join("node_modules");
    let installed_lock = fs::read_to_string(tooling_dir.join(INSTALLED_LOCK)).ok();
    let from_this_lock = installed_lock.as_deref() == Some(tooling.package_lock)
        && installed_versions(tooling, &node_modules).is_ok();
    let unmodified = from_this_lock
        && fs::read_to_string(tooling_dir.join(INSTALLED_TREE)).ok()
            == tree_fingerprint(&node_modules).ok();
    let needs_install = !unmodified;

    // Decided before anything is written, so a refused run leaves the tree untouched.
    if needs_install && !auto_install {
        let flag = match language {
            Language::Rust => "",
            Language::Ts => " --language ts",
        };
        let state = if from_this_lock {
            "codama's npm dependencies were changed after pinoc installed them (a file in node_modules was edited, added or removed, or a cache restored a different tree)."
        } else if node_modules.exists() {
            "codama's npm dependencies were not installed from this pinoc's lock file (another pinoc version installed them, or they were installed by hand)."
        } else {
            "codama's npm dependencies aren't installed yet."
        };
        anyhow::bail!(
            "{state}\n\n\
             Rerun with: pinoc client generate --generator codama{flag} --auto-install\n\n\
             That runs `npm ci` in {} with the exact package versions and lock file built into pinoc.",
            tooling_dir.display()
        );
    }

    fs::create_dir_all(&tooling_dir)
        .with_context(|| format!("Failed to create {}", tooling_dir.display()))?;
    ensure_gitignored(".pinoc-codama/")?;
    let script_path = tooling_dir.join("convert_and_render.mjs");
    fs::write(&script_path, script).with_context(|| "Failed to write conversion script")?;

    if needs_install {
        println!("📦 Installing codama's npm dependencies (npm ci)...");
        // Removed first, so an interrupted install is not mistaken for a finished one.
        let _ = fs::remove_file(tooling_dir.join(INSTALLED_LOCK));
        let _ = fs::remove_file(tooling_dir.join(INSTALLED_TREE));
        fs::write(tooling_dir.join("package.json"), tooling.package_json)
            .with_context(|| "Failed to write package.json")?;
        fs::write(tooling_dir.join("package-lock.json"), tooling.package_lock)
            .with_context(|| "Failed to write package-lock.json")?;
        // `npm ci` installs exactly what the lock file names, or fails.
        let status = Command::new("npm")
            .arg("ci")
            .current_dir(&tooling_dir)
            .status()
            .with_context(|| "Failed to run 'npm ci'")?;
        if !status.success() {
            anyhow::bail!("'npm ci' failed with exit code: {:?}", status.code());
        }
        fs::write(
            tooling_dir.join(INSTALLED_TREE),
            tree_fingerprint(&node_modules)?,
        )
        .with_context(|| format!("Failed to write {INSTALLED_TREE}"))?;
        fs::write(tooling_dir.join(INSTALLED_LOCK), tooling.package_lock)
            .with_context(|| format!("Failed to write {INSTALLED_LOCK}"))?;
    }
    println!("📦 {}", installed_versions(tooling, &node_modules)?);

    let src_dir = out_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    let idl_path_abs = fs::canonicalize(idl_path)
        .with_context(|| format!("Failed to resolve {}", idl_path.display()))?;
    let out_dir_abs = fs::canonicalize(out_dir)
        .with_context(|| format!("Failed to resolve {}", out_dir.display()))?;

    let mut render = Command::new("node");
    render.arg(&script_path).arg(&idl_path_abs);
    match language {
        Language::Rust => {
            render
                .arg(out_dir_abs.join("src").join("generated"))
                .arg(&out_dir_abs);
        }
        Language::Ts => {
            render.arg(&out_dir_abs);
        }
    }
    let status = render
        .status()
        .with_context(|| "Failed to run codama render script")?;
    if !status.success() {
        anyhow::bail!("codama render failed with exit code: {:?}", status.code());
    }

    // Neither renderer emits explicit enum discriminants.
    let generated_dir = src_dir.join("generated");
    match language {
        Language::Rust => discriminants::patch_rust(&generated_dir, enums)?,
        Language::Ts => discriminants::patch_ts(&generated_dir, enums)?,
    }

    match language {
        Language::Rust => {
            fs::write(
                out_dir.join("Cargo.toml"),
                cargo_toml(derives_addresses(&generated_dir)),
            )?;
            fs::write(src_dir.join("lib.rs"), lib_rs(&src_dir.join("generated")))?;
        }
        Language::Ts => {
            // The rendered package.json names `src/index.ts` as its entry point.
            let index = src_dir.join("index.ts");
            if !index.exists() {
                fs::write(index, "export * from './generated';\n")?;
            }
        }
    }
    Ok(())
}

/// A fingerprint of every file under `dir`: its relative path and contents, or
/// link target, in sorted order. FNV-1a, which is enough to notice a change; it
/// is not a defence against someone who can also rewrite the recorded value.
fn tree_fingerprint(dir: &Path) -> Result<String> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() && !path.is_symlink() {
                walk(&path, root, out)?;
            } else {
                out.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(dir, dir, &mut files)?;
    files.sort();

    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for relative in &files {
        let path = dir.join(relative);
        feed(relative.to_string_lossy().as_bytes());
        feed(&[0]);
        if path.is_symlink() {
            feed(fs::read_link(&path)?.to_string_lossy().as_bytes());
        } else {
            feed(&fs::read(&path)?);
        }
        feed(&[0]);
    }
    Ok(format!("{} files, fnv1a64 {hash:016x}\n", files.len()))
}

/// The installed version of each package pinoc asked for. Errors if one is not
/// the pinned version, which the lock file should make impossible.
fn installed_versions(tooling: &Tooling, node_modules: &Path) -> Result<String> {
    let mut found = Vec::new();
    for (name, pinned) in tooling.packages() {
        let manifest = node_modules.join(&name).join("package.json");
        let installed: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&manifest)
                .with_context(|| format!("Failed to read {}", manifest.display()))?,
        )?;
        let version = installed["version"].as_str().unwrap_or_default();
        if version != pinned {
            anyhow::bail!(
                "{name} {version} is installed in {}, but this pinoc renders with {pinned}",
                node_modules.display()
            );
        }
        found.push(format!("{name} {version}"));
    }
    Ok(found.join(", "))
}

/// Appends `pattern` to the project root's `.gitignore` if present, or creates
/// one if `cwd` is a git repo (never creates one otherwise).
fn ensure_gitignored(pattern: &str) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let gitignore_path = cwd.join(".gitignore");
    let gitignore_exists = gitignore_path.exists();

    if !gitignore_exists && !cwd.join(".git").exists() {
        return Ok(());
    }

    let existing = fs::read_to_string(&gitignore_path).unwrap_or_default();
    if existing
        .lines()
        .any(|line| line.trim().trim_end_matches('/') == pattern.trim_end_matches('/'))
    {
        return Ok(());
    }

    let mut updated = existing;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(pattern);
    updated.push('\n');
    fs::write(&gitignore_path, updated)
        .with_context(|| format!("Failed to update {}", gitignore_path.display()))?;
    println!("📝 Added {pattern} to .gitignore");
    Ok(())
}

fn check_node_available() -> Result<()> {
    let node_ok = Command::new("node")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let npm_ok = Command::new("npm")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if node_ok && npm_ok {
        return Ok(());
    }

    anyhow::bail!(
        "Node.js/npm not found, required for the codama client generator.\n\n\
         Quick install:\n  \
         curl -fsSL https://fnm.vercel.app/install | bash   # installs fnm (Node version manager)\n  \
         fnm install --lts && fnm use lts-latest\n\n\
         Or download an installer directly: https://nodejs.org/en/download\n\n\
         Once installed, rerun: pinoc client generate --generator codama"
    );
}

/// The real Codama renderer only emits `accounts/`, `instructions/`, and
/// `types/` when the IDL actually has content in that category (a program
/// with no accounts gets no `accounts/mod.rs` at all), so re-exporting them
/// unconditionally would fail to compile. `errors`/`programs` are always
/// rendered since every IDL has at least one program.
fn lib_rs(generated_dir: &Path) -> String {
    let mut reexports = String::new();
    for module in ["accounts", "instructions", "types"] {
        if generated_dir.join(module).join("mod.rs").exists() {
            reexports.push_str(&format!("pub use generated::{module}::*;\n"));
        }
    }

    format!(
        "#![allow(warnings)]\n\
         //! This code was generated by `pinoc client generate --generator codama`.\n\
         //! Do not edit by hand; rerun the command instead.\n\
         \n\
         pub mod generated;\n\
         pub use generated::*;\n\
         pub use generated::errors::*;\n\
         pub use generated::programs::*;\n\
         {reexports}"
    )
}

/// Whether the rendered client derives program addresses (PDA helpers).
fn derives_addresses(generated_dir: &Path) -> bool {
    fn scan(dir: &Path) -> bool {
        let Ok(entries) = fs::read_dir(dir) else {
            return false;
        };
        entries.flatten().any(|entry| {
            let path = entry.path();
            if path.is_dir() {
                return scan(&path);
            }
            fs::read_to_string(&path).is_ok_and(|src| {
                src.contains("find_program_address") || src.contains("create_program_address")
            })
        })
    }
    scan(generated_dir)
}

/// `solana-pubkey` only has the address-derivation functions behind its
/// `curve25519` feature, so it is enabled when the client uses them.
fn cargo_toml(derives_addresses: bool) -> String {
    let solana_pubkey = if derives_addresses {
        r#"solana-pubkey = { version = "4.2", features = ["curve25519"] }"#
    } else {
        r#"solana-pubkey = "4.2""#
    };
    r#"[package]
name = "codama-client"
version = "0.1.0"
edition = "2021"

[dependencies]
borsh = { version = "1", features = ["derive"] }
solana-address = { version = "2.6", features = ["decode", "borsh"] }
SOLANA_PUBKEY
solana-instruction = "3.4"
solana-account-info = "3"
solana-program-error = "3"
solana-cpi = "3"
thiserror = "2"
num-derive = "0.4"
num-traits = "0.2"
"#
    .replace("SOLANA_PUBKEY", solana_pubkey)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_exact(version: &str) -> bool {
        let parts: Vec<&str> = version.split('.').collect();
        parts.len() == 3
            && parts
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    }

    #[test]
    fn npm_packages_are_pinned_and_locked() {
        for tooling in [&RUST_TOOLING, &TS_TOOLING] {
            let lock: serde_json::Value = serde_json::from_str(tooling.package_lock).unwrap();
            let packages = tooling.packages();
            assert_eq!(packages.len(), 3);
            for (name, version) in packages {
                assert!(
                    is_exact(&version),
                    "{name} is not pinned exactly: {version}"
                );
                // The lock file agrees with the manifest, as `npm ci` requires.
                assert_eq!(
                    lock["packages"][""]["dependencies"][&name], version,
                    "{name}"
                );
                assert_eq!(
                    lock["packages"][format!("node_modules/{name}")]["version"],
                    version,
                    "{name}"
                );
            }
            // Every package in the tree is fixed by an integrity hash.
            for (path, entry) in lock["packages"].as_object().unwrap() {
                if !path.is_empty() {
                    assert!(
                        entry["integrity"].is_string(),
                        "{path} has no integrity hash"
                    );
                }
            }
        }
    }
}
