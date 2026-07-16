//! Shells out to the real Codama JS pipeline to render a Rust client. Node.js
//! deps live in a project-local `<out_dir>/.pinoc-codama/`, installed only
//! with explicit consent (`--auto-install`), never silently.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;

const PACKAGE_JSON: &str = r#"{
  "name": "pinoc-codama-tooling",
  "private": true,
  "type": "module",
  "dependencies": {
    "codama": "^1.5.0",
    "@codama/nodes-from-anchor": "^1.3.8",
    "@codama/renderers-rust": "^1.2.9"
  }
}
"#;

const CONVERT_SCRIPT: &str = r#"import { rootNodeFromAnchor } from '@codama/nodes-from-anchor';
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

/// Requires Node.js/npm.
pub fn generate_via_codama(idl_path: &Path, out_dir: &Path, auto_install: bool) -> Result<()> {
    check_node_available()?;

    let tooling_dir = out_dir.join(".pinoc-codama");
    fs::create_dir_all(&tooling_dir)
        .with_context(|| format!("Failed to create {}", tooling_dir.display()))?;
    fs::write(tooling_dir.join("package.json"), PACKAGE_JSON)
        .with_context(|| "Failed to write package.json")?;
    ensure_gitignored(".pinoc-codama/")?;
    let script_path = tooling_dir.join("convert_and_render.mjs");
    fs::write(&script_path, CONVERT_SCRIPT).with_context(|| "Failed to write conversion script")?;

    if !tooling_dir.join("node_modules").exists() {
        if !auto_install {
            anyhow::bail!(
                "codama's npm dependencies aren't installed yet.\n\n\
                 Install them with:\n  npm install --prefix {}\n\n\
                 Or rerun with: pinoc client generate --generator codama --auto-install",
                tooling_dir.display()
            );
        }
        println!("📦 Installing codama (first run only)...");
        let status = Command::new("npm")
            .arg("install")
            .current_dir(&tooling_dir)
            .status()
            .with_context(|| "Failed to run 'npm install'")?;
        if !status.success() {
            anyhow::bail!("'npm install' failed with exit code: {:?}", status.code());
        }
    }

    let src_dir = out_dir.join("src");
    let generated_dir = src_dir.join("generated");
    fs::create_dir_all(&src_dir)?;

    let idl_path_abs = fs::canonicalize(idl_path)
        .with_context(|| format!("Failed to resolve {}", idl_path.display()))?;
    let out_dir_abs = fs::canonicalize(out_dir)
        .with_context(|| format!("Failed to resolve {}", out_dir.display()))?;
    let generated_dir_abs = out_dir_abs.join("src").join("generated");

    let status = Command::new("node")
        .arg(&script_path)
        .arg(&idl_path_abs)
        .arg(&generated_dir_abs)
        .arg(&out_dir_abs)
        .status()
        .with_context(|| "Failed to run codama render script")?;
    if !status.success() {
        anyhow::bail!("codama render failed with exit code: {:?}", status.code());
    }

    fs::write(out_dir.join("Cargo.toml"), cargo_toml())?;
    fs::write(src_dir.join("lib.rs"), lib_rs(&generated_dir))?;
    Ok(())
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

fn cargo_toml() -> String {
    r#"[package]
name = "codama-client"
version = "0.1.0"
edition = "2021"

[dependencies]
borsh = { version = "1", features = ["derive"] }
solana-address = { version = "2.6", features = ["decode", "borsh"] }
solana-pubkey = "4.2"
solana-instruction = "3.4"
solana-account-info = "3"
solana-program-error = "3"
solana-cpi = "3"
thiserror = "2"
num-derive = "0.4"
num-traits = "0.2"
"#
    .to_string()
}
