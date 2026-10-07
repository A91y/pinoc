mod common;

use common::{fixture, pinoc, stdout};
use serde_json::Value;

/// `pinoc check --json` findings, sorted so the order does not depend on the
/// order the filesystem lists source files in.
fn findings(fixture_name: &str, args: &[&str]) -> Vec<Value> {
    let mut all = vec!["check", "--json"];
    all.extend(args);
    let out = pinoc(&fixture(fixture_name), &all);
    sorted(serde_json::from_str(&stdout(&out)).expect("--json stdout is a JSON array"))
}

fn sorted(mut findings: Vec<Value>) -> Vec<Value> {
    findings.sort_by_key(|f| {
        (
            f["span"]["file"].as_str().unwrap().to_string(),
            f["span"]["line"].as_u64().unwrap(),
            f["code"].as_str().unwrap().to_string(),
        )
    });
    findings
}

fn codes(findings: &[Value]) -> Vec<&str> {
    findings
        .iter()
        .map(|f| f["code"].as_str().unwrap())
        .collect()
}

/// `(code, evidence)` of the findings in one file, in source order.
fn in_file<'a>(findings: &'a [Value], file: &str) -> Vec<(&'a str, &'a str)> {
    findings
        .iter()
        .filter(|f| f["span"]["file"] == file)
        .map(|f| (f["code"].as_str().unwrap(), f["evidence"].as_str().unwrap()))
        .collect()
}

#[test]
fn free_function_output_matches_0_2_5() {
    let dir = fixture("check_free");
    let expected: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(dir.join("expected/check.json")).unwrap())
            .unwrap();
    assert_eq!(findings("check_free", &[]), sorted(expected));

    // The human rendering lists files in filesystem order, so compare line sets.
    let lines = |s: &str| {
        let mut v: Vec<String> = s.lines().map(str::to_string).collect();
        v.sort();
        v
    };
    let expected = std::fs::read_to_string(dir.join("expected/check.txt")).unwrap();
    let actual = stdout(&pinoc(&dir, &["check"]));
    assert_eq!(lines(&actual), lines(&expected));
}

#[test]
fn impl_methods_are_analysed_like_free_functions() {
    // Every lint, including the heuristic one hidden at the default threshold.
    let args = ["--deny", "all"];
    let free = findings("check_free", &args);
    let methods = findings("check_impl", &args);

    let expected = in_file(&free, "src/handlers.rs");
    assert_eq!(
        expected.iter().map(|f| f.0).collect::<Vec<_>>(),
        ["ACC001-P", "ZC002-P", "ACC002-P", "ACC003-P", "CPI001-P"]
    );
    assert_eq!(in_file(&methods, "src/handlers.rs"), expected);
    assert_eq!(codes(&methods), codes(&free));
}

#[test]
fn zero_handlers_is_reported_not_passed_as_clean() {
    let dir = fixture("check_no_handlers");
    let out = pinoc(&dir, &["check"]);
    let text = stdout(&out);
    assert!(text.contains("NO-HANDLERS"), "{text}");
    assert!(text.contains("not a clean result"), "{text}");
    assert!(!text.contains("No issues found"), "{text}");
    // The struct-layout lints do not depend on handlers and still run.
    assert!(text.contains("ZC001-P"), "{text}");
    assert_eq!(out.status.code(), Some(0));

    assert_eq!(
        codes(&findings("check_no_handlers", &[])),
        ["NO-HANDLERS", "ZC001-P"]
    );
    for deny in ["all", "NO-HANDLERS"] {
        let out = pinoc(&dir, &["check", "--deny", deny]);
        assert_eq!(out.status.code(), Some(1), "--deny {deny}");
    }
    let out = pinoc(&dir, &["check", "--allow", "NO-HANDLERS", "ZC001-P"]);
    assert!(stdout(&out).contains("No issues found"));
}

#[test]
fn missing_src_is_reported() {
    let dir = fixture("check_no_handlers").join("src");
    let text = stdout(&pinoc(&dir, &["check"]));
    assert!(text.contains("NO-HANDLERS"), "{text}");
    assert!(text.contains("no `src/` directory"), "{text}");
}

/// `(code, file, line)` of each finding, and the evidence of the one at `index`.
fn located(findings: &[Value]) -> Vec<(&str, &str, u64)> {
    findings
        .iter()
        .map(|f| {
            (
                f["code"].as_str().unwrap(),
                f["span"]["file"].as_str().unwrap(),
                f["span"]["line"].as_u64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn typed_context_and_pinocchio_0_11_names() {
    let found = findings("check_typed_context", &[]);
    // `process` reads `vault` through `self.accounts`, with no owner or length
    // check anywhere; the two free functions use the 0.11 accessor names.
    assert_eq!(
        located(&found),
        [
            ("ACC001-P", "src/lib.rs", 40),
            ("ZC002-P", "src/lib.rs", 40),
            ("ZC002-P", "src/lib.rs", 53),
            ("ACC001-P", "src/lib.rs", 64),
        ]
    );
    let evidence = found[0]["evidence"].as_str().unwrap();
    assert!(
        evidence.contains("(in `Deposit::process`, bound in `DepositAccounts::try_from`)"),
        "{evidence}"
    );
}

#[test]
fn accounts_are_followed_from_try_from_into_process_and_helpers() {
    let found = findings("check_context", &[]);
    assert_eq!(
        located(&found),
        [
            // Stored, and never read back through the type.
            ("UNTRACKED-ACCOUNTS", "src", 0),
            // Owner checked through a helper, bytes given to an unbounded cast.
            ("ZC002-P", "src/instructions/set_fee.rs", 20),
            // `try_from` checks nothing; `process` loads without an owner check
            // and compares the authority's address without requiring a signature.
            ("ACC001-P", "src/instructions/withdraw.rs", 27),
            ("ACC002-P", "src/instructions/withdraw.rs", 28),
            // Read through a local of the instruction type, in another file.
            ("ACC001-P", "src/processor.rs", 5),
        ]
    );
    // `deposit` (checks through helpers) and `close` (the same checks inline)
    // are in the fixture and produce nothing.

    let evidence = |index: usize| found[index]["evidence"].as_str().unwrap();
    assert!(
        evidence(0).contains("`ArchiveAccounts::try_from`"),
        "{}",
        evidence(0)
    );
    assert!(!evidence(0).contains("Deposit"), "{}", evidence(0));
    assert!(
        evidence(2).contains("account `vault`")
            && evidence(2)
                .contains("(in `Withdraw::process`, bound in `WithdrawAccounts::try_from`)"),
        "{}",
        evidence(2)
    );
    assert!(evidence(3).contains("account `admin`"), "{}", evidence(3));
    assert!(
        evidence(4).contains("(in `process_sweep`, bound in `SweepAccounts::try_from`)"),
        "{}",
        evidence(4)
    );
}

#[test]
fn instructions_sharing_an_account_list_are_checked_separately() {
    let found = findings("check_shared_accounts", &[]);
    assert_eq!(
        located(&found),
        [
            // `process_pairs` takes its accounts in chunks.
            ("UNTRACKED-ACCOUNTS", "src", 0),
            // Named in a `match` arm, and leaving a `match` through a `let`.
            ("ACC001-P", "src/batch.rs", 6),
            ("ACC001-P", "src/batch.rs", 27),
            // Read by `Unwind` through a method of `Sell`, whose own `check`
            // `Unwind` never calls.
            ("ACC001-P", "src/sell.rs", 63),
            // A `for` loop over the slice, and `.last()`.
            ("ACC001-P", "src/tail.rs", 24),
            ("ACC001-P", "src/tail.rs", 33),
            ("ACC002-P", "src/unwind.rs", 25),
        ]
    );
    // `Sell` loads the same accounts by splitting the slice and is silent, as
    // is `expect_closed`, which takes each account with `.get(at).ok_or(..)?`.

    let evidence = |index: usize| found[index]["evidence"].as_str().unwrap();
    assert!(
        evidence(0).contains("`process_pairs` (binds no account from its slice)"),
        "{}",
        evidence(0)
    );
    assert!(evidence(2).contains("account `oracle`"), "{}", evidence(2));
    assert!(
        evidence(3).contains(
            "(in `Sell::pool_reserve`, as part of `Unwind`, bound in `SellAccounts::load`)"
        ),
        "{}",
        evidence(3)
    );
    assert!(evidence(6).contains("account `admin`"), "{}", evidence(6));
}

#[test]
fn a_project_can_name_its_own_authorities() {
    let dir = common::temp_copy("check_shared_accounts");
    std::fs::write(
        dir.join("Pinoc.toml"),
        "[provider]\ncluster = \"localhost\"\nwallet = \"~/.config/solana/id.json\"\n\n[check]\nauthority_names = [\"keeper\"]\n",
    )
    .unwrap();
    let out = pinoc(&dir, &["check", "--json"]);
    let found = sorted(serde_json::from_str(&stdout(&out)).unwrap());
    let signer: Vec<(&str, u64)> = found
        .iter()
        .filter(|f| f["code"] == "ACC002-P")
        .map(|f| {
            (
                f["span"]["file"].as_str().unwrap(),
                f["span"]["line"].as_u64().unwrap(),
            )
        })
        .collect();
    // `keeper` joins `admin`; `Sell`, which requires both signatures, stays silent.
    assert_eq!(signer, [("src/unwind.rs", 25), ("src/unwind.rs", 26)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_misspelt_check_key_or_section_is_refused() {
    let provider = "[provider]\ncluster = \"localhost\"\nwallet = \"~/.config/solana/id.json\"\n\n";
    for (config, named) in [
        (
            "[check]\nautority_names = [\"keeper\"]\n",
            "unknown field `autority_names`",
        ),
        (
            "[chek]\nauthority_names = [\"keeper\"]\n",
            "unknown field `chek`",
        ),
    ] {
        let dir = common::temp_copy("check_shared_accounts");
        std::fs::write(dir.join("Pinoc.toml"), format!("{provider}{config}")).unwrap();
        let out = pinoc(&dir, &["check"]);
        assert!(!out.status.success(), "{config}");
        let text = common::stderr(&out);
        assert!(text.contains(named), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn a_check_value_that_cannot_be_applied_is_refused() {
    let provider = "[provider]\ncluster = \"localhost\"\nwallet = \"~/.config/solana/id.json\"\n\n";
    for (config, named) in [
        (
            "[check]\nconfidence_threshold = \"possible\"\n",
            "`confidence_threshold = \"possible\"` under `[check]` in Pinoc.toml is not a confidence level",
        ),
        (
            "[check]\nallow = [\"ACC001\"]\n",
            "`ACC001`, given to `allow` under `[check]` in Pinoc.toml, is not a lint code",
        ),
    ] {
        let dir = common::temp_copy("check_shared_accounts");
        std::fs::write(dir.join("Pinoc.toml"), format!("{provider}{config}")).unwrap();
        let out = pinoc(&dir, &["check"]);
        assert!(!out.status.success(), "{config}");
        let text = common::stderr(&out);
        assert!(text.contains(named), "{text}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    let out = pinoc(
        &fixture("check_shared_accounts"),
        &["check", "--deny", "ACC9"],
    );
    assert!(!out.status.success());
    let text = common::stderr(&out);
    assert!(
        text.contains("`ACC9`, given to `--deny`, is not a lint code"),
        "{text}"
    );
}

#[test]
fn the_hidden_findings_line_names_their_codes() {
    let dir = common::temp_copy("check_shared_accounts");
    std::fs::write(
        dir.join("Pinoc.toml"),
        "[provider]\ncluster = \"localhost\"\nwallet = \"~/.config/solana/id.json\"\n\n[check]\nconfidence_threshold = \"definite\"\n",
    )
    .unwrap();
    let text = stdout(&pinoc(&dir, &["check"]));
    assert!(
        text.contains(
            "lower-confidence findings (ACC001-P, ACC002-P, ACC003-P) below the `definite` threshold hidden"
        ),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
