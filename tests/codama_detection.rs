mod common;

use common::{pinoc, read_json, stderr, stdout, temp_copy};
use std::path::PathBuf;

const NATIVE: &str = "native Codama extraction (Codama macros detected)";
const SHIM: &str = "shank IDL + compatibility shim";

/// A copy of the Codama fixture whose `codama = "0.9"` dependency line is
/// replaced by `dependency` (TOML appended to the manifest).
fn project(dependency: &str) -> PathBuf {
    let dir = temp_copy("idl_codama_errors");
    let manifest = dir.join("Cargo.toml");
    let original = std::fs::read_to_string(&manifest).unwrap();
    let without = original.replace("codama = \"0.9\"\n", "");
    assert_ne!(without, original);
    std::fs::write(&manifest, format!("{without}\n{dependency}\n")).unwrap();
    dir
}

fn idl(dir: &std::path::Path) -> String {
    let out = pinoc(dir, &["idl"]);
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    stdout(&out)
}

fn is_native(dir: &std::path::Path) -> bool {
    read_json(&dir.join("target/idl/idl_codama_errors.codama.json"))["kind"] == "rootNode"
}

#[test]
fn codama_dependency_output_matches_0_3_0() {
    let dir = temp_copy("idl_codama_errors");
    assert!(idl(&dir).contains(NATIVE));
    for file in ["idl_codama_errors.json", "idl_codama_errors.codama.json"] {
        assert_eq!(
            std::fs::read_to_string(dir.join("target/idl").join(file)).unwrap(),
            std::fs::read_to_string(dir.join("expected").join(file)).unwrap(),
            "{file}"
        );
    }
}

#[test]
fn codama_macros_dependency_is_detected() {
    let dir = project("[dependencies.codama-macros]\nversion = \"0.9.3\"");
    assert!(idl(&dir).contains(NATIVE));
    assert!(is_native(&dir));

    // No `-y`: detection agrees with the choice, so the run gets as far as the
    // npm dependencies (or all the way, where they are installed).
    let out = pinoc(&dir, &["client", "generate", "--generator", "codama"]);
    let err = stderr(&out);
    assert!(!err.contains("Refusing to generate"), "{err}");
    assert!(
        out.status.success() || err.contains("npm dependencies aren't installed"),
        "{err}"
    );
}

#[test]
fn codama_under_a_target_table_or_renamed_is_detected() {
    for dependency in [
        "[target.'cfg(not(target_os = \"solana\"))'.dependencies]\ncodama-macros = \"0.9.3\"",
        "[target.'cfg(not(target_os = \"solana\"))'.dependencies]\ncodama = \"0.9.3\"",
        "[dependencies.macros]\npackage = \"codama-macros\"\nversion = \"0.9.3\"",
    ] {
        let dir = project(dependency);
        assert!(idl(&dir).contains(NATIVE), "{dependency}");
        assert!(is_native(&dir), "{dependency}");
    }
}

#[test]
fn derives_without_a_codama_dependency_take_the_shim_and_say_why() {
    let dir = project("");
    let out = idl(&dir);
    assert!(out.contains(SHIM), "{out}");
    assert!(
        out.contains(
            "Codama derives found, but neither `codama` nor `codama-macros` is a dependency"
        ),
        "{out}"
    );
    assert!(!is_native(&dir));

    let refused = pinoc(&dir, &["client", "generate", "--generator", "codama"]);
    assert!(stderr(&refused).contains("Codama derives were found, but neither"));
}

#[test]
fn no_derives_keeps_the_existing_message() {
    let dir = temp_copy("client_program");
    let out = idl(&dir);
    assert!(
        out.contains("shank IDL + compatibility shim (no Codama macros detected)"),
        "{out}"
    );
}

#[test]
fn a_different_codama_minor_is_reported() {
    let lock = |version: &str| {
        format!("version = 3\n\n[[package]]\nname = \"codama-macros\"\nversion = \"{version}\"\n")
    };

    let dir = project("[dependencies.codama-macros]\nversion = \"0.13\"");
    std::fs::write(dir.join("Cargo.lock"), lock("0.13.2")).unwrap();
    let out = idl(&dir);
    assert!(out.contains("Codama versions differ"), "{out}");
    assert!(out.contains("codama-macros 0.13.2"), "{out}");

    // Without a lock file the manifest requirement is used.
    std::fs::remove_file(dir.join("Cargo.lock")).unwrap();
    assert!(idl(&dir).contains("codama-macros 0.13"));

    std::fs::write(dir.join("Cargo.lock"), lock("0.9.1")).unwrap();
    assert!(!idl(&dir).contains("Codama versions differ"));

    // A directive the bundled extractor does not have stops the run and says why.
    std::fs::write(dir.join("Cargo.lock"), lock("0.13.2")).unwrap();
    let lib = dir.join("src/lib.rs");
    let source = std::fs::read_to_string(&lib).unwrap();
    let with_new = source.replace(
        "    pub authority: [u8; 32],",
        "    #[codama(display(skip))]\n    pub authority: [u8; 32],",
    );
    assert_ne!(with_new, source);
    std::fs::write(&lib, with_new).unwrap();
    let failed = pinoc(&dir, &["idl"]);
    assert!(!failed.status.success());
    let err = stderr(&failed);
    assert!(
        err.contains("unrecognized codama directive `display`"),
        "{err}"
    );
    assert!(err.contains("--> src/lib.rs:12:14"), "{err}");
    assert!(err.contains("#[codama(display(skip))]"), "{err}");
    assert_eq!(
        err.matches("unrecognized codama directive").count(),
        1,
        "{err}"
    );
    assert!(
        err.contains("this program uses codama-macros 0.13.2"),
        "{err}"
    );
}

#[test]
fn bundled_codama_constant_matches_the_lock_file() {
    let lock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.lock")).unwrap();
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/idl/codama_native.rs"
    ))
    .unwrap();
    let locked = lock
        .split("[[package]]")
        .find(|p| p.contains("name = \"codama\"\n"))
        .and_then(|p| p.split("version = \"").nth(1)?.split('"').next())
        .unwrap();
    assert!(
        source.contains(&format!("BUNDLED_CODAMA: &str = \"{locked}\"")),
        "BUNDLED_CODAMA is not {locked}"
    );
}
