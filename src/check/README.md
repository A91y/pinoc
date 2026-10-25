# `pinoc check` internals

How to use the command is in [docs/check.md](../../docs/check.md). This page is for changing it.

## Rules the code keeps

- **The contract is frozen once released.** Codes, ids, `Category` values and the JSON shape of `Finding` (`contract.rs`) are a permanent compatibility surface. A new one is an addition; name it in the CHANGELOG.
- **Precision over recall.** When the engine cannot follow an account (a call outside the crate, an ambiguous path), it marks the account delegated and the lints stay quiet. Do not guess through indirection.
- **Findings go through `Finding`.** No lint prints. Configuration, suppression, the threshold and the exit code are applied once, in `mod.rs`.
- **Facts are added with their reader.** A `Validation` or `Use` variant lands in the same change as the lint that reads it.
- **A heuristic finding never fails CI by default.**

## Adding a lint

1. Write `lints/<code>_<name>.rs` implementing `Lint`:
   - per-file checks in `run`;
   - account and CPI checks in `run_handlers`, over the facts from `facts/`;
   - `summary()` for the one-line description;
   - `explain()` as `include_str!` of its doc.
2. Register it in `lints::registry()`. `codes::all()` then picks it up for config validation, `--explain` and SARIF.
3. Write `docs/lints/<CODE>.md` in the existing shape: a title, the severity line, what counts, limits, the fix, then a `## Flagged` and a `## Not flagged` example. The test suite runs both examples through the binary.
4. Add its row to the table in [docs/check.md](../../docs/check.md#codes). A test fails when a registered code has no row or no doc.
5. Add fixtures and tests through the binary in `tests/check.rs`.

## Module map

| Path | Responsibility |
| --- | --- |
| `mod.rs` | Discovers and parses source, runs lints, adds the coverage findings, applies config severity, suppression, and the confidence threshold, sets the exit code. |
| `contract.rs` | `Finding`, the `Lint` trait, and `Severity` / `Confidence` / `Category` / `Backend` / `Span`. The JSON shape is frozen here. |
| `suppress.rs` | Parses `// pinoc:allow(CODE)` comments and matches them to findings. |
| `codes.rs` | Every code the command can report (the registered lints, then `NO-HANDLERS`, `UNTRACKED-ACCOUNTS` and `UNUSED-ALLOW`), with its id, defaults, summary and `docs/lints` text. Config validation, `--explain` and the SARIF rules read it. |
| `output.rs` | The `human`, `json`, `github` and `sarif` renderers. |
| `locate.rs` | A finding's path from the repository root and 1-based column, for `github` and `sarif`. |
| `facts/mod.rs` | Per-account fact table, built over the whole crate: for each handler, how each account is validated and used, including through helper functions and from the functions that read stored accounts back, plus the program-id source of each `invoke`. Account/CPI lints run on this. |
| `lints/mod.rs` | The lint registry and span helpers. |
| `lints/acc001_owner.rs`, `lints/acc002_signer.rs`, `lints/acc003_confusion.rs`, `lints/cpi001_arbitrary_cpi.rs`, `lints/zc001_padding.rs`, `lints/zc002_length.rs`, `lints/zc003_repr_c.rs`, `lints/disc001_discriminator.rs` | The individual checks. |
| [`docs/lints/<CODE>.md`](../../docs/lints/) | One entry per code: what it catches, how, its limits, the fix, and a flagged and a clean example. The test suite runs the examples through `pinoc check`. |
