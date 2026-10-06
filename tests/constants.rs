mod common;

use common::{pinoc, read_json, stderr, stdout, temp_copy};
use serde_json::json;

#[test]
fn marked_constants_reach_both_idls() {
    let dir = temp_copy("idl_constants_pda");
    let out = pinoc(&dir, &["idl"]);
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        stdout(&out).contains("3 constant(s) added to the IDL"),
        "{}",
        stdout(&out)
    );

    let number =
        |format: &str| json!({ "kind": "numberTypeNode", "format": format, "endian": "le" });
    let codama = read_json(&dir.join("target/idl/idl_constants_pda.codama.json"));
    assert_eq!(
        codama["program"]["constants"],
        json!([
            {
                "kind": "constantNode",
                "name": "maxOpenPositions",
                "docs": ["Open positions a trader may hold."],
                "type": number("u8"),
                "value": { "kind": "numberValueNode", "number": 8 },
            },
            {
                "kind": "constantNode",
                "name": "maxAtas",
                "type": number("u8"),
                "value": { "kind": "numberValueNode", "number": 12 },
            },
            {
                "kind": "constantNode",
                "name": "maxReserveFloor",
                "docs": ["Ceiling on the reserve floor."],
                "type": number("u64"),
                "value": { "kind": "numberValueNode", "number": 100_000_000_000_000u64 },
            },
        ])
    );

    let shank = read_json(&dir.join("target/idl/idl_constants_pda.json"));
    assert_eq!(
        shank["constants"],
        json!([
            { "name": "MAX_OPEN_POSITIONS", "type": "u8", "value": "8" },
            { "name": "MAX_ATAS", "type": "u8", "value": "12" },
            { "name": "MAX_RESERVE_FLOOR", "type": "u64", "value": "100000000000000" },
        ])
    );
}

#[test]
fn a_program_without_markers_has_no_constants_key() {
    let dir = temp_copy("client_program");
    let out = pinoc(&dir, &["idl"]);
    assert!(out.status.success());
    assert!(!stdout(&out).contains("constant(s) added"));
    let shank = read_json(&dir.join("target/idl/client_program.json"));
    assert!(shank.get("constants").is_none());
}

#[test]
fn a_marker_pinoc_cannot_honour_is_an_error() {
    for (source, expected) in [
        (
            "// pinoc:constant\npub const SLOTS: usize = 4;",
            "its type `usize` is not a fixed-width integer",
        ),
        (
            "// pinoc:constant(u8)\npub const BIG: u64 = 300;",
            "value 300 does not fit in `u8`",
        ),
        (
            "// pinoc:constant\npub const DERIVED: u64 = compute();",
            "not an integer expression pinoc can evaluate",
        ),
        (
            "// pinoc:constant\npub const HUGE: u64 = 1 << 60;",
            "beyond 2^53",
        ),
        (
            "// pinoc:constant(string)\npub const NAME: u8 = 1;",
            "is not an integer type",
        ),
        (
            "// pinoc:constant\n\npub const GAP: u8 = 1;",
            "is not directly above a `const` item",
        ),
        (
            "// pinoc:constant\n\n/// Docs.\npub const GAP: u8 = 1;",
            "is not directly above a `const` item",
        ),
        (
            "// pinoc:constant\npub fn not_a_const() {}",
            "is not directly above a `const` item",
        ),
    ] {
        let dir = temp_copy("client_program");
        let lib = dir.join("src/lib.rs");
        let original = std::fs::read_to_string(&lib).unwrap();
        std::fs::write(&lib, format!("{original}\n{source}\n")).unwrap();
        let out = pinoc(&dir, &["idl"]);
        assert!(!out.status.success(), "{source}");
        assert!(
            stderr(&out).contains(expected),
            "{source}: {}",
            stderr(&out)
        );
        assert!(!dir.join("target/idl").exists(), "{source}");
    }
}

#[test]
fn a_marker_may_sit_above_or_below_the_doc_comments() {
    for source in [
        "// pinoc:constant\n/// Docs.\n#[allow(dead_code)]\npub const LIMIT: u8 = 3;",
        "/// Docs.\n// pinoc:constant\n#[allow(dead_code)]\npub const LIMIT: u8 = 3;",
        // Below the doc comments the marker is inside the item, so blank lines
        // around it are harmless.
        "/// Docs.\n\n// pinoc:constant\npub const LIMIT: u8 = 3;",
        "/// Docs.\n// pinoc:constant\n\npub const LIMIT: u8 = 3;",
    ] {
        let dir = temp_copy("client_program");
        let lib = dir.join("src/lib.rs");
        let original = std::fs::read_to_string(&lib).unwrap();
        std::fs::write(&lib, format!("{original}\n{source}\n")).unwrap();
        let out = pinoc(&dir, &["idl"]);
        assert!(out.status.success(), "{source}: {}", stderr(&out));
        let shank = read_json(&dir.join("target/idl/client_program.json"));
        assert_eq!(
            shank["constants"],
            json!([{ "name": "LIMIT", "type": "u8", "value": "3" }]),
            "{source}"
        );
    }
}
