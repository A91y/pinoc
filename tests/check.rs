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

/// One case per file: a passing table, a wrong byte, a reordered variant, a
/// row/variant count mismatch, a non-literal row, the single form, snake_case,
/// and a wrong row under an inline allow.
#[test]
fn discriminator_constants_are_checked_against_their_preimages() {
    let all = findings("check_discriminators", &[]);
    let disc: Vec<(&str, u64, &str, Option<&str>)> = all
        .iter()
        .filter(|f| f["code"] == "DISC001-P")
        .map(|f| {
            assert_eq!(f["id"], "wrong-discriminator");
            assert_eq!(f["severity"], "deny");
            assert_eq!(f["confidence"], "definite");
            (
                f["span"]["file"].as_str().unwrap(),
                f["span"]["line"].as_u64().unwrap(),
                f["evidence"].as_str().unwrap(),
                f["fix"].as_str(),
            )
        })
        .collect();
    let deposit = "[62, 205, 242, 175, 244, 169, 136, 52]";
    let withdraw = "[192, 241, 201, 217, 70, 150, 90, 247]";
    let vault = "[211, 8, 232, 43, 2, 152, 117, 119]";
    let pool = "[241, 154, 109, 4, 17, 177, 109, 188]";
    let fix = |b: &str| format!("replace it with {b}");
    let fixes = [fix(withdraw), fix(deposit), fix(pool)];
    assert_eq!(
        disc,
        [
            (
                "src/count.rs",
                9,
                "`// pinoc:discriminators` above `DISCRIMINATORS` cannot be checked: `EventKind` has 3 variants and the table 2 rows",
                None
            ),
            (
                "src/non_literal.rs",
                10,
                "`// pinoc:discriminators` above `DISCRIMINATORS` cannot be checked: a value is not an array literal of bytes",
                None
            ),
            (
                "src/reordered.rs",
                9,
                &*format!("`DISCRIMINATORS[0]` (`EventKind::Withdraw`) is {deposit}, but sha256(\"event:Withdraw\")[..8] is {withdraw}"),
                Some(&*fixes[0])
            ),
            (
                "src/reordered.rs",
                10,
                &*format!("`DISCRIMINATORS[1]` (`EventKind::Deposit`) is {withdraw}, but sha256(\"event:Deposit\")[..8] is {deposit}"),
                Some(&*fixes[1])
            ),
            (
                "src/single.rs",
                9,
                &*format!("`POOL` is {vault}, but sha256(\"account:Pool\")[..8] is {pool}"),
                Some(&*fixes[2])
            ),
            (
                "src/wrong_byte.rs",
                10,
                &*format!("`DISCRIMINATORS[1]` (`EventKind::Withdraw`) is [193, 241, 201, 217, 70, 150, 90, 247], but sha256(\"event:Withdraw\")[..8] is {withdraw}"),
                Some(&*fixes[0])
            ),
        ]
    );

    // The allow in allowed.rs was used, so it is not reported as unused.
    let text = stdout(&pinoc(&fixture("check_discriminators"), &["check"]));
    assert!(!text.contains("allowed.rs"), "{text}");
}

fn lint_docs() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/lints");
    let mut docs: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .map(|p| {
            (
                p.file_stem().unwrap().to_str().unwrap().to_string(),
                std::fs::read_to_string(&p).unwrap(),
            )
        })
        .collect();
    docs.sort();
    docs
}

#[test]
fn every_code_has_one_doc_and_a_readme_row_and_explain_prints_it() {
    let dir = fixture("check_no_handlers");
    let out = pinoc(&dir, &["check", "--explain", "NOPE"]);
    assert!(!out.status.success());
    let err = common::stderr(&out);
    let listed = err
        .lines()
        .next()
        .unwrap_or_default()
        .split("The codes are ")
        .nth(1)
        .unwrap_or_else(|| panic!("{err}"))
        .trim()
        .trim_end_matches('.');
    let mut codes: Vec<&str> = listed.split(", ").collect();
    codes.sort();

    let docs = lint_docs();
    let documented: Vec<&str> = docs.iter().map(|(c, _)| c.as_str()).collect();
    assert_eq!(codes, documented, "registered codes and docs/lints/ differ");

    let readme = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/check.md"),
    )
    .unwrap();
    for (code, text) in &docs {
        assert!(
            readme.contains(&format!("| `{code}` |")),
            "{code} has no row in docs/check.md"
        );
        assert!(text.starts_with(&format!("# {code} ")), "{code}.md title");
        let out = pinoc(&dir, &["check", "--explain", code]);
        assert!(out.status.success(), "{code}");
        assert_eq!(&stdout(&out), text, "--explain {code}");
    }
}

/// The `## Flagged` and `## Not flagged` examples in docs/lints, run through the
/// binary, so a doc cannot show an example the lint does not behave like.
#[test]
fn lint_doc_examples_behave_as_documented() {
    for (code, text) in lint_docs() {
        for (heading, expect) in [("## Flagged\n", true), ("## Not flagged\n", false)] {
            let Some(rest) = text.split(heading).nth(1) else {
                continue;
            };
            let block = rest
                .split("```rust\n")
                .nth(1)
                .and_then(|b| b.split("```").next())
                .unwrap_or_else(|| panic!("{code}: no rust block under {heading}"));
            let dir = std::env::temp_dir().join(format!(
                "pinoc-doc-example-{}-{code}-{expect}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("src")).unwrap();
            std::fs::write(dir.join("src/lib.rs"), block).unwrap();
            let found = codes(&findings_in(&dir, &["--deny", &code])).contains(&code.as_str());
            assert_eq!(found, expect, "{code} {heading}{block}");
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

fn findings_in(dir: &std::path::Path, args: &[&str]) -> Vec<Value> {
    let mut all = vec!["check", "--json"];
    all.extend(args);
    let out = pinoc(dir, &all);
    serde_json::from_str(&stdout(&out)).expect("--json stdout is a JSON array")
}

/// Runs `pinoc check` in `dir` with `GITHUB_WORKSPACE` set, as in Actions.
fn pinoc_in_workspace(
    dir: &std::path::Path,
    workspace: &std::path::Path,
    args: &[&str],
) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_pinoc"))
        .args(args)
        .current_dir(dir)
        .env("GITHUB_WORKSPACE", workspace)
        .output()
        .expect("failed to run pinoc")
}

fn sorted_lines(s: &str) -> Vec<String> {
    let mut v: Vec<String> = s.lines().map(str::to_string).collect();
    v.sort();
    v
}

#[test]
fn github_format_annotates_from_the_repository_root() {
    let dir = fixture("check_free");
    let workspace = dir.parent().unwrap();
    let out = pinoc_in_workspace(&dir, workspace, &["check", "--format", "github"]);
    let expected = std::fs::read_to_string(dir.join("expected/check.github.txt")).unwrap();
    // Annotations come in filesystem order, so compare line sets.
    assert_eq!(sorted_lines(&stdout(&out)), sorted_lines(&expected));
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn github_format_puts_project_wide_findings_on_the_manifest() {
    let dir = fixture("check_no_handlers");
    let out = pinoc_in_workspace(
        &dir,
        dir.parent().unwrap(),
        &["check", "--format", "github"],
    );
    let text = stdout(&out);
    assert!(
        text.lines()
            .any(|l| l.starts_with("::warning title=NO-HANDLERS no-handlers::")),
        "{text}"
    );

    let copy = common::temp_copy("check_no_handlers");
    std::fs::write(copy.join("Cargo.toml"), "[package]\nname = \"x\"\n").unwrap();
    let out = pinoc_in_workspace(
        &copy,
        copy.parent().unwrap(),
        &["check", "--format", "github"],
    );
    let name = copy.file_name().unwrap().to_str().unwrap();
    assert!(
        stdout(&out).lines().any(|l| l.starts_with(&format!(
            "::warning file={name}/Cargo.toml,line=1,col=1,title=NO-HANDLERS no-handlers::"
        ))),
        "{}",
        stdout(&out)
    );
}

#[test]
fn every_format_exits_like_the_human_one() {
    for name in [
        "check_context",
        "check_discriminators",
        "check_free",
        "check_impl",
        "check_no_handlers",
        "check_shared_accounts",
        "check_typed_context",
    ] {
        let dir = fixture(name);
        let human = pinoc(&dir, &["check"]);
        for format in ["json", "github", "sarif"] {
            let out = pinoc(&dir, &["check", "--format", format]);
            assert_eq!(
                out.status.code(),
                human.status.code(),
                "{name} --format {format}"
            );
            assert_eq!(
                common::stderr(&out),
                common::stderr(&human),
                "{name} --format {format}"
            );
        }
        assert_eq!(
            stdout(&pinoc(&dir, &["check", "--json"])),
            stdout(&pinoc(&dir, &["check", "--format", "json"])),
            "{name}"
        );
    }
}

#[test]
fn sarif_lists_every_code_as_a_rule_and_matches_the_json_findings() {
    let dir = fixture("check_free");
    let out = pinoc_in_workspace(&dir, dir.parent().unwrap(), &["check", "--format", "sarif"]);
    let sarif: Value = serde_json::from_str(&stdout(&out)).expect("SARIF is JSON");
    assert_eq!(sarif["version"], "2.1.0");
    assert_eq!(
        sarif["$schema"],
        "https://json.schemastore.org/sarif-2.1.0.json"
    );
    let run = &sarif["runs"][0];
    assert_eq!(run["tool"]["driver"]["name"], "pinoc");

    let rules: Vec<&str> = run["tool"]["driver"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    let mut sorted_rules = rules.clone();
    sorted_rules.sort();
    let docs = lint_docs();
    assert_eq!(
        sorted_rules,
        docs.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>()
    );
    for rule in run["tool"]["driver"]["rules"].as_array().unwrap() {
        let code = rule["id"].as_str().unwrap();
        let doc = &docs.iter().find(|(c, _)| c == code).unwrap().1;
        assert_eq!(rule["help"]["markdown"], *doc, "{code}");
    }

    let json = findings("check_free", &[]);
    let mut from_sarif: Vec<(String, String, u64, u64)> = run["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let index = r["ruleIndex"].as_u64().unwrap() as usize;
            assert_eq!(rules[index], r["ruleId"].as_str().unwrap());
            let loc = &r["locations"][0]["physicalLocation"];
            (
                r["ruleId"].as_str().unwrap().to_string(),
                r["level"].as_str().unwrap().to_string(),
                loc["region"]["startLine"].as_u64().unwrap(),
                loc["region"]["startColumn"].as_u64().unwrap(),
            )
        })
        .collect();
    let mut from_json: Vec<(String, String, u64, u64)> = json
        .iter()
        .map(|f| {
            let level = match f["severity"].as_str().unwrap() {
                "deny" => "error",
                "warn" => "warning",
                _ => "note",
            };
            (
                f["code"].as_str().unwrap().to_string(),
                level.to_string(),
                f["span"]["line"].as_u64().unwrap(),
                f["span"]["col"].as_u64().unwrap() + 1,
            )
        })
        .collect();
    from_sarif.sort();
    from_json.sort();
    assert_eq!(from_sarif, from_json);
    assert!(run["results"]
        .as_array()
        .unwrap()
        .iter()
        .all(
            |r| r["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                .as_str()
                .unwrap()
                .starts_with("check_free/src/")
        ));
}
