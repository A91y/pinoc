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

/// Needs Node.js and network access: `cargo test -- --ignored`.
#[test]
#[ignore]
fn codama_rust_client_handles_an_instruction_without_accounts() {
    let dir = temp_copy("idl_codama_tail");
    assert!(pinoc(&dir, &["idl"]).status.success());
    let out = pinoc(
        &dir,
        &[
            "client",
            "generate",
            "--generator",
            "codama",
            "--auto-install",
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let ping =
        std::fs::read_to_string(dir.join("clients/rust-codama/src/generated/instructions/ping.rs"))
            .unwrap();
    assert!(!ping.contains("NaN"), "{ping}");

    let out = pinoc(
        &dir,
        &["client", "generate", "--language", "ts", "--auto-install"],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let settle =
        std::fs::read_to_string(dir.join("clients/ts/src/generated/instructions/settle.ts"))
            .unwrap();
    assert!(settle.contains("positions: Array<"), "{settle}");
}

const USE_DISCRIMINANT: &str = "#[borsh(use_discriminant = true)]";

#[test]
fn shank_client_keeps_explicit_enum_discriminants() {
    let dir = temp_copy("client_enum_values");
    assert!(pinoc(&dir, &["idl", "--idl-generator", "shank"])
        .status
        .success());
    let out = pinoc(&dir, &["client", "generate", "--generator", "shank", "-y"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(stdout(&out).contains("Enums with explicit discriminants (`Status`)"));

    let types = dir.join("clients/rust-shank/src/generated/types");
    let status = std::fs::read_to_string(types.join("status.rs")).unwrap();
    assert!(
        status.contains(&format!(
            "{USE_DISCRIMINANT}\npub enum Status {{\n    Open = 1,\n    Active = 2,\n    Closed = 5,\n}}"
        )),
        "{status}"
    );
    // An enum without explicit values is rendered as before.
    let plain = std::fs::read_to_string(types.join("plain.rs")).unwrap();
    assert!(
        plain.contains("pub enum Plain {\n    A,\n    B,\n}"),
        "{plain}"
    );
    assert!(!plain.contains(USE_DISCRIMINANT));
}

#[test]
fn enums_that_cannot_be_corrected_stop_the_codama_client() {
    for (source, expected) in [
        (
            "#[derive(codama_macros::CodamaType)]\npub enum Shape { Circle { r: u8 } = 3, Square = 7 }",
            "enum `shape` has variants with data and explicit discriminators (3, 7)",
        ),
        (
            "#[derive(codama_macros::CodamaAccount)]\npub struct Holder {\n    #[codama(type = enum(variant(name = \"a\", discriminator = 4)))]\n    pub kind: u8,\n}",
            "declared inline",
        ),
    ] {
        let dir = temp_copy("client_enum_values");
        let header = "pinocchio::address::declare_id!(\"EfPyA5fx11YAY9XwmrWVddcQe5bBZeNsKA8avqYUH6Qr\");\n";
        std::fs::write(dir.join("src/lib.rs"), format!("{header}{source}\n")).unwrap();
        let idl = pinoc(&dir, &["idl", "--idl-generator", "codama"]);
        if !idl.status.success() {
            // The attribute form is not available in this Codama release.
            assert!(expected == "declared inline", "{}", stderr(&idl));
            continue;
        }
        let out = pinoc(&dir, &["client", "generate", "--generator", "codama", "-y"]);
        assert!(!out.status.success(), "{source}");
        assert!(stderr(&out).contains(expected), "{}", stderr(&out));
        assert!(!dir.join("clients").exists(), "{source}");
    }
}

/// Needs Node.js and network access: `cargo test -- --ignored`.
#[test]
#[ignore]
fn codama_clients_keep_explicit_enum_discriminants() {
    let dir = temp_copy("client_enum_values");
    assert!(pinoc(&dir, &["idl"]).status.success());
    for language in ["ts", "rust"] {
        let out = pinoc(
            &dir,
            &[
                "client",
                "generate",
                "--generator",
                "codama",
                "--language",
                language,
                "--auto-install",
            ],
        );
        assert!(out.status.success(), "{language}: {}", stderr(&out));
        assert!(stdout(&out).contains("Enums with explicit discriminants (`status`)"));
    }

    let ts = dir.join("clients/ts/src/generated/types");
    let status = std::fs::read_to_string(ts.join("status.ts")).unwrap();
    assert!(
        status.contains("export enum Status {\n  Open = 1,\n  Active = 2,\n  Closed = 5,\n}"),
        "{status}"
    );
    for codec in ["getEnumEncoder", "getEnumDecoder"] {
        assert!(
            status.contains(&format!(
                "{codec}(Status, {{ useValuesAsDiscriminators: true }})"
            )),
            "{status}"
        );
    }
    let plain = std::fs::read_to_string(ts.join("plain.ts")).unwrap();
    assert!(plain.contains("getEnumEncoder(Plain)"), "{plain}");

    let rust = dir.join("clients/rust-codama/src/generated/types");
    let status = std::fs::read_to_string(rust.join("status.rs")).unwrap();
    assert!(
        status.contains(&format!(
            "{USE_DISCRIMINANT}\npub enum Status {{\nOpen = 1,\nActive = 2,\nClosed = 5,\n}}"
        )),
        "{status}"
    );
    let plain = std::fs::read_to_string(rust.join("plain.rs")).unwrap();
    assert!(!plain.contains(USE_DISCRIMINANT), "{plain}");
}

/// Needs Node.js and network access: `cargo test -- --ignored`.
#[test]
#[ignore]
fn codama_clients_for_a_program_with_pdas_and_constants() {
    let dir = temp_copy("idl_constants_pda");
    assert!(pinoc(&dir, &["idl"]).status.success());
    for language in ["rust", "ts"] {
        let out = pinoc(
            &dir,
            &[
                "client",
                "generate",
                "--generator",
                "codama",
                "--language",
                language,
                "--auto-install",
            ],
        );
        assert!(out.status.success(), "{language}: {}", stderr(&out));
    }

    // The PDA helpers need `curve25519`, which solana-pubkey does not enable by default.
    let rust = dir.join("clients/rust-codama");
    let vault = std::fs::read_to_string(rust.join("src/generated/accounts/vault.rs")).unwrap();
    assert!(vault.contains("find_program_address"), "{vault}");
    let manifest = std::fs::read_to_string(rust.join("Cargo.toml")).unwrap();
    assert!(
        manifest.contains(r#"solana-pubkey = { version = "4.2", features = ["curve25519"] }"#),
        "{manifest}"
    );

    let constants =
        std::fs::read_to_string(dir.join("clients/ts/src/generated/constants/idlConstantsPda.ts"))
            .unwrap();
    assert!(
        constants.contains("export const MAX_ATAS: number = 12;"),
        "{constants}"
    );
    assert!(
        constants.contains("export const MAX_RESERVE_FLOOR: bigint = 100000000000000n;"),
        "{constants}"
    );

    // A client with no PDA keeps the plain dependency.
    let dir = temp_copy("client_enum_values");
    assert!(pinoc(&dir, &["idl"]).status.success());
    let out = pinoc(
        &dir,
        &[
            "client",
            "generate",
            "--generator",
            "codama",
            "--auto-install",
        ],
    );
    assert!(out.status.success(), "{}", stderr(&out));
    let manifest = std::fs::read_to_string(dir.join("clients/rust-codama/Cargo.toml")).unwrap();
    assert!(manifest.contains("solana-pubkey = \"4.2\"\n"), "{manifest}");
}
