mod common;

use common::{pinoc, stderr, stdout, temp_copy};
use std::path::Path;
use std::process::Command;

/// A git repo holding a fixture with its IDL already generated.
fn project_with_idl() -> std::path::PathBuf {
    let dir = temp_copy("client_program");
    std::fs::write(dir.join(".gitignore"), "/target\n").unwrap();
    assert!(pinoc(&dir, &["idl"]).status.success());
    let git = |args: &[&str]| {
        let ok = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .expect("git is required for this test")
            .status
            .success();
        assert!(ok, "git {args:?}");
    };
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&[
        "-c",
        "user.email=t@t",
        "-c",
        "user.name=t",
        "commit",
        "-qm",
        "init",
    ]);
    dir
}

fn git_status(dir: &Path) -> String {
    let out = Command::new("git")
        .args(["status", "--short"])
        .current_dir(dir)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn codama_without_deps_fails_and_leaves_the_tree_clean() {
    let dir = project_with_idl();
    for language in ["rust", "ts"] {
        let out = pinoc(
            &dir,
            &[
                "client",
                "generate",
                "--generator",
                "codama",
                "-y",
                "--language",
                language,
            ],
        );
        assert!(!out.status.success(), "{language}");
        assert_eq!(git_status(&dir), "", "{language}: {}", stderr(&out));
        assert_eq!(
            std::fs::read_to_string(dir.join(".gitignore")).unwrap(),
            "/target\n"
        );
        assert!(!dir.join("clients").exists(), "{language}");
    }
}

#[test]
fn typescript_needs_the_codama_generator() {
    let dir = project_with_idl();
    let out = pinoc(
        &dir,
        &[
            "client",
            "generate",
            "--generator",
            "shank",
            "--language",
            "ts",
        ],
    );
    assert!(!out.status.success());
    assert!(stderr(&out).contains("--language ts needs --generator codama"));
    assert_eq!(git_status(&dir), "");
}

#[test]
fn shank_rust_client_is_the_default() {
    let dir = project_with_idl();
    let out = pinoc(&dir, &["client", "generate", "--generator", "shank"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("Rust client written to clients/rust-shank/"));
    assert!(dir.join("clients/rust-shank/src/lib.rs").exists());
}

/// Installs npm packages, so it needs Node.js and network access:
/// `cargo test -- --ignored`.
#[test]
#[ignore]
fn codama_renders_typescript_and_rust() {
    let dir = project_with_idl();
    let generate = |language: &str| {
        let out = pinoc(
            &dir,
            &[
                "client",
                "generate",
                "--language",
                language,
                "--generator",
                "codama",
                "-y",
                "--auto-install",
            ],
        );
        assert!(out.status.success(), "{language}: {}", stderr(&out));
    };

    generate("ts");
    let gitignore = std::fs::read_to_string(dir.join(".gitignore")).unwrap();
    assert!(
        gitignore.lines().any(|l| l == ".pinoc-codama/"),
        "{gitignore}"
    );
    let instructions = dir.join("clients/ts/src/generated/instructions");
    let idl = common::read_json(&dir.join("target/idl/client_program.json"));
    let index = std::fs::read_to_string(instructions.join("index.ts")).unwrap();
    let idl_instructions = idl["instructions"].as_array().unwrap();
    assert_eq!(idl_instructions.len(), 2);
    for ix in idl_instructions {
        let name = ix["name"].as_str().unwrap();
        let file = format!("{}{}", name[..1].to_lowercase(), &name[1..]);
        assert!(
            instructions.join(format!("{file}.ts")).exists(),
            "{file}.ts"
        );
        assert!(index.contains(&format!("./{file}")), "{index}");
    }
    assert!(dir
        .join("clients/ts/src/generated/errors/index.ts")
        .exists());
    assert!(dir.join("clients/ts/package.json").exists());

    generate("rust");
    assert!(dir.join("clients/rust-codama/Cargo.toml").exists());
    assert!(dir
        .join("clients/rust-codama/src/generated/errors/mod.rs")
        .exists());
}
