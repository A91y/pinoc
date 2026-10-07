mod common;

use common::{pinoc, read_json, stdout, temp_copy};

#[test]
fn manual_errors_reach_native_codama_idl() {
    let dir = temp_copy("idl_manual_errors");
    let out = pinoc(&dir, &["idl"]);
    assert!(out.status.success(), "{}", stdout(&out));
    assert!(stdout(&out).contains("native Codama extraction"));

    let idl = read_json(&dir.join("target/idl/idl_manual_errors.codama.json"));
    assert_eq!(idl["kind"], "rootNode");
    assert_eq!(
        idl["program"]["errors"],
        serde_json::json!([
            { "kind": "errorNode", "name": "invalidAuthority", "code": 0, "message": "Invalid Authority" },
            { "kind": "errorNode", "name": "counterOverflow", "code": 6, "message": "Counter Overflow" },
            { "kind": "errorNode", "name": "notInitialized", "code": 7, "message": "Not Initialized" },
        ])
    );
    // The account Codama extracted itself is still there.
    assert_eq!(idl["program"]["accounts"][0]["name"], "counter");
}

#[test]
fn native_codama_errors_are_not_overridden() {
    let dir = temp_copy("idl_codama_errors");
    let out = pinoc(&dir, &["idl"]);
    assert!(out.status.success(), "{}", stdout(&out));
    assert!(!stdout(&out).contains("from the shank IDL"));

    let idl = read_json(&dir.join("target/idl/idl_codama_errors.codama.json"));
    let errors = idl["program"]["errors"].as_array().unwrap();
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0]["message"], "Authority does not match the counter");
}

#[test]
fn shank_path_output_matches_0_2_5() {
    let dir = temp_copy("idl_manual_errors");
    let out = pinoc(&dir, &["idl", "--idl-generator", "shank"]);
    assert!(out.status.success(), "{}", stdout(&out));
    for file in ["idl_manual_errors.json", "idl_manual_errors.codama.json"] {
        assert_eq!(
            std::fs::read_to_string(dir.join("target/idl").join(file)).unwrap(),
            std::fs::read_to_string(dir.join("expected").join(file)).unwrap(),
            "{file}"
        );
    }
}

#[test]
fn native_codama_error_codes_follow_the_from_impl() {
    let dir = temp_copy("idl_codama_errors");
    let lib = dir.join("src/lib.rs");
    let source = std::fs::read_to_string(&lib).unwrap();
    let offset = source
        .replace("    CounterOverflow,", "    CounterOverflow = 10,")
        .replace("Custom(e as u32)", "Custom(e as u32 + 6000)");
    assert_ne!(offset, source);
    std::fs::write(&lib, offset).unwrap();

    let out = pinoc(&dir, &["idl"]);
    assert!(out.status.success(), "{}", stdout(&out));
    let codes = |file: &str, errors: fn(&serde_json::Value) -> &serde_json::Value| {
        let idl = read_json(&dir.join("target/idl").join(file));
        errors(&idl)
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["code"].as_u64().unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        codes("idl_codama_errors.json", |idl| &idl["errors"]),
        [6000, 6010]
    );
    assert_eq!(
        codes("idl_codama_errors.codama.json", |idl| &idl["program"]
            ["errors"]),
        [6000, 6010]
    );
}

/// Runs `pinoc idl` on a Codama crate whose `lib.rs` body is `source`; returns
/// stdout and the `(name, code, message)` of each native error.
fn native_errors(source: &str) -> (String, Vec<(String, u64, String)>) {
    let dir = temp_copy("idl_codama_errors");
    let header =
        "pinocchio::address::declare_id!(\"EfPyA5fx11YAY9XwmrWVddcQe5bBZeNsKA8avqYUH6Qr\");\n";
    std::fs::write(dir.join("src/lib.rs"), format!("{header}{source}")).unwrap();
    let out = pinoc(&dir, &["idl"]);
    assert!(out.status.success(), "{}", stdout(&out));
    let idl = read_json(&dir.join("target/idl/idl_codama_errors.codama.json"));
    assert_eq!(idl["kind"], "rootNode");
    let errors = idl["program"]["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["name"].as_str().unwrap().to_string(),
                e["code"].as_u64().unwrap(),
                e["message"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    (stdout(&out), errors)
}

const OFFSET_FROM: &str = "
impl From<MyError> for ProgramError {
    fn from(e: MyError) -> Self { ProgramError::Custom(e as u32 + 6000) }
}";

#[test]
fn native_codes_convert_when_inflections_disagree() {
    // Codama and heck split the one-letter word in `NotAMint` differently.
    let (out, errors) = native_errors(&format!(
        "
#[derive(codama::CodamaErrors)]
pub enum MyError {{
    InvalidDiscriminator,
    NotAMint = 10,
    IsATarget,
    AMintAuthority = 20,
}}
{OFFSET_FROM}"
    ));
    let codes: Vec<u64> = errors.iter().map(|e| e.1).collect();
    assert_eq!(codes, [6000, 6010, 6011, 6020], "{errors:?}");
    assert!(out.contains("error codes converted the same way"), "{out}");
    assert!(!out.contains("⚠️"), "{out}");

    let messages: Vec<&str> = errors.iter().map(|e| e.2.as_str()).collect();
    assert_eq!(
        messages,
        [
            "Invalid Discriminator",
            "Not A Mint",
            "Is A Target",
            "A Mint Authority"
        ]
    );
}

#[test]
fn native_codes_that_cannot_be_matched_are_reported() {
    // The `From` impl converts a different enum than the one Codama extracted.
    let (out, errors) = native_errors(&format!(
        "
#[derive(codama::CodamaErrors)]
pub enum OtherError {{
    Unrelated,
    AlsoUnrelated,
}}
pub enum MyError {{
    First,
}}
{OFFSET_FROM}"
    ));
    assert_eq!(errors.iter().map(|e| e.1).collect::<Vec<_>>(), [0, 1]);
    assert!(
        out.contains("2 error code(s) left as raw discriminants"),
        "{out}"
    );
    assert!(out.contains("`unrelated`, `alsoUnrelated`"), "{out}");
}

#[test]
fn native_codes_convert_the_variants_that_match() {
    let (out, errors) = native_errors(&format!(
        "
#[derive(codama::CodamaErrors)]
pub enum MyError {{
    First,
    Second,
}}
#[derive(codama::CodamaErrors)]
pub enum ExtraError {{
    Stray = 40,
}}
{OFFSET_FROM}"
    ));
    let by_name = |name: &str| errors.iter().find(|e| e.0 == name).unwrap().1;
    assert_eq!(by_name("first"), 6000);
    assert_eq!(by_name("second"), 6001);
    assert_eq!(by_name("stray"), 40);
    assert!(out.contains("2 of 3 error codes converted"), "{out}");
    assert!(
        out.contains("1 error code(s) left as raw discriminants"),
        "{out}"
    );
    assert!(out.contains("`stray`"), "{out}");
}

#[test]
fn native_names_that_collide_after_normalising_are_not_guessed() {
    // Codama emits one `notAmint` node for both variants. Two variants share its
    // key, so it is left raw and reported, not given the first one's code.
    let (out, errors) = native_errors(&format!(
        "
#[derive(codama::CodamaErrors)]
pub enum MyError {{
    First,
    NotAMint,
    NotAmint,
}}
{OFFSET_FROM}"
    ));
    assert_eq!(errors.iter().map(|e| e.1).collect::<Vec<_>>(), [6000, 2]);
    assert!(
        out.contains("1 error code(s) left as raw discriminants"),
        "{out}"
    );
    assert!(out.contains("`notAmint`"), "{out}");
}

#[test]
fn forced_native_extraction_without_macros_says_where_errors_come_from() {
    let dir = temp_copy("client_program");
    let out = pinoc(&dir, &["idl", "--idl-generator", "codama"]);
    let text = stdout(&out);
    assert!(text.contains("found no instructions or accounts"), "{text}");
    assert!(
        text.contains("1 error(s) in .codama.json come from the shank IDL"),
        "{text}"
    );
    assert!(!text.contains("No `CodamaErrors` found"), "{text}");
    let idl = read_json(&dir.join("target/idl/client_program.codama.json"));
    assert_eq!(idl["program"]["errors"][0]["name"], "ownerMismatch");
}

#[test]
fn codama_0_13_directives_and_restored_empty_lists() {
    let dir = temp_copy("idl_codama_tail");
    let out = pinoc(&dir, &["idl"]);
    assert!(
        out.status.success(),
        "{}{}",
        stdout(&out),
        common::stderr(&out)
    );
    let idl = read_json(&dir.join("target/idl/idl_codama_tail.codama.json"));
    let instruction = |name: &str| {
        idl["program"]["instructions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["name"] == name)
            .unwrap()
            .clone()
    };

    // A variable account tail.
    assert_eq!(
        instruction("settle")["remainingAccounts"],
        serde_json::json!([{
            "kind": "instructionRemainingAccountsNode",
            "isWritable": true,
            "value": { "kind": "argumentValueNode", "name": "positions" },
        }])
    );

    // codama 0.13 omits empty lists; the renderers need them present.
    assert_eq!(instruction("ping")["accounts"], serde_json::json!([]));
    assert_eq!(idl["additionalPrograms"], serde_json::json!([]));
    for list in ["constants", "definedTypes", "errors", "events", "pdas"] {
        assert_eq!(idl["program"][list], serde_json::json!([]), "{list}");
    }

    // An array whose Rust length is a const, declared through the attribute.
    let tiers = &idl["program"]["accounts"][0]["data"]["fields"][1];
    assert_eq!(tiers["name"], "tiers");
    assert_eq!(tiers["type"]["kind"], "arrayTypeNode");
    assert_eq!(tiers["type"]["count"]["value"], 4);
}

#[test]
fn an_error_variant_can_be_named_explicitly() {
    let (out, errors) = native_errors(&format!(
        "
#[derive(codama::CodamaErrors)]
pub enum MyError {{
    InvalidAuthority,
    #[codama(name = \"notAMint\")]
    NotAMint,
    IsATarget,
}}
{OFFSET_FROM}"
    ));
    // The named variant takes its name; the others keep Codama's conversion.
    // Codes and messages stay paired with their variants.
    assert_eq!(
        errors,
        [
            (
                "invalidAuthority".to_string(),
                6000,
                "Invalid Authority".to_string()
            ),
            ("notAMint".to_string(), 6001, "Not A Mint".to_string()),
            ("isAtarget".to_string(), 6002, "Is A Target".to_string()),
        ]
    );
    assert!(
        out.contains("1 error name(s) in .codama.json taken from"),
        "{out}"
    );

    // A name that is a different word altogether keeps its code and message.
    let (_, errors) = native_errors(&format!(
        "
#[derive(codama::CodamaErrors)]
pub enum MyError {{
    InvalidAuthority,
    #[codama(name = \"wrongKindOfMint\")]
    NotAMint = 80,
}}
{OFFSET_FROM}"
    ));
    assert_eq!(
        errors[1],
        (
            "wrongKindOfMint".to_string(),
            6080,
            "Not A Mint".to_string()
        )
    );
}

#[test]
fn an_unusable_error_name_is_refused() {
    for (variants, expected) in [
        (
            "#[codama(name = \"Not A Mint\")]\n    NotAMint,",
            "is not a camelCase name",
        ),
        (
            "First,\n    #[codama(name = \"first\")]\n    Second,",
            "collides with variant `First`",
        ),
        (
            "#[codama(name = \"same\")]\n    First,\n    #[codama(name = \"same\")]\n    Second,",
            "collides with variant",
        ),
    ] {
        let dir = temp_copy("idl_codama_errors");
        let source = format!(
            "pinocchio::address::declare_id!(\"EfPyA5fx11YAY9XwmrWVddcQe5bBZeNsKA8avqYUH6Qr\");\n#[derive(codama::CodamaErrors)]\npub enum MyError {{\n    {variants}\n}}\n"
        );
        std::fs::write(dir.join("src/lib.rs"), source).unwrap();
        let out = pinoc(&dir, &["idl"]);
        assert!(!out.status.success(), "{variants}");
        assert!(
            common::stderr(&out).contains(expected),
            "{variants}: {}",
            common::stderr(&out)
        );
    }
}

#[test]
fn a_skipped_error_variant_is_reported() {
    let (out, errors) = native_errors(&format!(
        "
#[derive(codama::CodamaErrors)]
pub enum MyError {{
    InvalidAuthority,
    #[codama(skip)]
    #[codama(name = \"notAMint\")]
    NotAMint,
    IsATarget,
}}
{OFFSET_FROM}"
    ));
    // The variant is left out, its name with it, and the codes around it stay put.
    assert_eq!(
        errors,
        [
            (
                "invalidAuthority".to_string(),
                6000,
                "Invalid Authority".to_string()
            ),
            ("isAtarget".to_string(), 6002, "Is A Target".to_string()),
        ]
    );
    assert!(
        out.contains("1 error variant(s) left out of .codama.json by `#[codama(skip)]`: NotAMint"),
        "{out}"
    );
    assert!(!out.contains("error name(s) in .codama.json"), "{out}");
}
