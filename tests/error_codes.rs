mod common;

use common::{pinoc, read_json, stdout};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const UNKNOWN_WARNING: &str = "emitted as raw enum discriminants";

/// Runs `pinoc idl` on a one-file crate whose `lib.rs` is `source`, and returns
/// the command's stdout and the `(name, code)` pairs of the shank IDL.
fn idl_errors(source: &str) -> (String, Vec<(String, u64)>) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let dir: PathBuf = std::env::temp_dir().join(format!(
        "pinoc-test-{}-errors-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"errs\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\ncrate-type = [\"cdylib\", \"lib\"]\n",
    )
    .unwrap();
    let header =
        "pinocchio::address::declare_id!(\"EfPyA5fx11YAY9XwmrWVddcQe5bBZeNsKA8avqYUH6Qr\");\n";
    std::fs::write(dir.join("src/lib.rs"), format!("{header}{source}")).unwrap();

    let out = pinoc(&dir, &["idl"]);
    assert!(out.status.success(), "{}", stdout(&out));
    let idl = read_json(&dir.join("target/idl/errs.json"));
    let errors = idl["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["name"].as_str().unwrap().to_string(),
                e["code"].as_u64().unwrap(),
            )
        })
        .collect();

    // The codama-compatible IDL carries the same codes.
    let codama = read_json(&dir.join("target/idl/errs.codama.json"));
    assert_eq!(codama["errors"], idl["errors"]);
    (stdout(&out), errors)
}

fn codes(errors: &[(String, u64)]) -> Vec<u64> {
    errors.iter().map(|e| e.1).collect()
}

const ENUM: &str = "
pub enum MyError {
    First,
    Second,
    Tenth = 10,
    Eleventh,
    Twentieth = 20,
}
";

#[test]
fn plain_discriminant_cast_is_unchanged() {
    let (out, errors) = idl_errors(&format!(
        "{ENUM}
impl From<MyError> for ProgramError {{
    fn from(e: MyError) -> Self {{ ProgramError::Custom(e as u32) }}
}}"
    ));
    assert_eq!(codes(&errors), [0, 1, 10, 11, 20]);
    assert!(
        !out.contains("offset") && !out.contains(UNKNOWN_WARNING),
        "{out}"
    );
}

#[test]
fn literal_offset() {
    let (out, errors) = idl_errors(&format!(
        "{ENUM}
impl From<MyError> for ProgramError {{
    fn from(e: MyError) -> Self {{ ProgramError::Custom(e as u32 + 6000) }}
}}"
    ));
    assert_eq!(codes(&errors), [6000, 6001, 6010, 6011, 6020]);
    assert!(out.contains("+6000 offset"), "{out}");
}

#[test]
fn named_constant_offset() {
    let (_, errors) = idl_errors(&format!(
        "{ENUM}
mod consts {{
    pub const THOUSAND: u32 = 1_000;
    pub const ERROR_BASE: u32 = 6 * THOUSAND;
}}
impl From<MyError> for ProgramError {{
    fn from(value: MyError) -> Self {{
        Self::Custom(consts::ERROR_BASE + (value as u32))
    }}
}}"
    ));
    assert_eq!(codes(&errors), [6000, 6001, 6010, 6011, 6020]);
}

#[test]
fn offset_through_a_method_on_the_enum() {
    let (_, errors) = idl_errors(&format!(
        "{ENUM}
pub const ERROR_BASE: u32 = 6_000;
impl MyError {{
    pub const fn code(self) -> u32 {{
        self as u32 + ERROR_BASE
    }}
}}
impl From<MyError> for ProgramError {{
    fn from(e: MyError) -> Self {{ ProgramError::Custom(e.code()) }}
}}"
    ));
    assert_eq!(codes(&errors), [6000, 6001, 6010, 6011, 6020]);
}

#[test]
fn associated_constant_and_method_returning_program_error() {
    let (_, errors) = idl_errors(&format!(
        "{ENUM}
impl MyError {{
    const BASE: u32 = 100;
    fn to_program_error(self) -> ProgramError {{
        return ProgramError::Custom(self as u32 + Self::BASE);
    }}
}}
impl From<MyError> for ProgramError {{
    fn from(e: MyError) -> Self {{ e.to_program_error() }}
}}"
    ));
    assert_eq!(codes(&errors), [100, 101, 110, 111, 120]);
}

#[test]
fn match_with_a_code_per_variant() {
    let (out, errors) = idl_errors(&format!(
        "{ENUM}
impl From<MyError> for ProgramError {{
    fn from(e: MyError) -> Self {{
        ProgramError::Custom(match e {{
            MyError::First => 7,
            MyError::Second => 8,
            MyError::Tenth => 70,
            MyError::Eleventh => 71,
            MyError::Twentieth => 90,
        }})
    }}
}}"
    ));
    assert_eq!(codes(&errors), [7, 8, 70, 71, 90]);
    assert!(out.contains("taken from the `match`"), "{out}");
}

#[test]
fn unrecognised_conversion_warns_and_keeps_discriminants() {
    for body in [
        "ProgramError::Custom(lookup(e))",
        "ProgramError::Custom(e as u32 * 2)",
        "ProgramError::Custom(match e { MyError::First => 7, _ => 9 })",
        "let code = e as u32 + 6000; ProgramError::Custom(code)",
    ] {
        let (out, errors) = idl_errors(&format!(
            "{ENUM}
impl From<MyError> for ProgramError {{
    fn from(e: MyError) -> Self {{ {body} }}
}}"
        ));
        assert_eq!(codes(&errors), [0, 1, 10, 11, 20], "{body}");
        assert!(out.contains(UNKNOWN_WARNING), "{body}: {out}");
    }
}

#[test]
fn thiserror_enum_gets_the_same_offset() {
    let source = |conversion: &str| {
        format!(
            "
#[derive(Debug, thiserror::Error)]
pub enum MyError {{
    #[error(\"first failed\")]
    First,
    #[error(\"second failed\")]
    Second,
}}
impl From<MyError> for ProgramError {{
    fn from(e: MyError) -> Self {{ ProgramError::Custom({conversion}) }}
}}"
        )
    };
    let (_, plain) = idl_errors(&source("e as u32"));
    assert_eq!(codes(&plain), [0, 1]);
    let (out, shifted) = idl_errors(&source("e as u32 + 6000"));
    assert_eq!(codes(&shifted), [6000, 6001]);
    assert!(out.contains("+6000 offset"), "{out}");
}

#[test]
fn thiserror_enum_with_a_from_impl_for_another_enum_warns() {
    let (out, errors) = idl_errors(
        "
#[derive(Debug, thiserror::Error)]
pub enum ReportedError {
    #[error(\"first failed\")]
    First,
}
pub enum OtherError {
    Only,
}
impl From<OtherError> for ProgramError {
    fn from(e: OtherError) -> Self { ProgramError::Custom(e as u32 + 6000) }
}",
    );
    assert_eq!(codes(&errors), [0]);
    assert!(out.contains("converts a different enum"), "{out}");
}
