# `pinoc check`

`pinoc check` lints a Pinocchio program for Solana protocol invariants that rustc, clippy and rust-analyzer do not model:
- account ownership and signer checks;
- cross-program invocation safety;
- zero-copy memory layout;
- discriminator constants.

A program that drains a vault is usually valid Rust that compiles clean; that gap is what it covers. What clippy or rustc already catch (arithmetic overflow, lossy casts, `unwrap`, ignored `Result`) is out of scope, and so is a missing writable check, since the runtime rejects writes to accounts not marked writable.

It parses every `.rs` file under `src/`, runs each lint, applies configuration and suppression, prints the findings, and exits `1` if a surviving finding is `deny`.

```bash
pinoc check                      # report findings, exit nonzero on a deny
pinoc check --deny '*'           # every code fails
pinoc check --format github      # annotate the pull request in GitHub Actions
pinoc check --explain ACC001-P   # what a code means, with a flagged and a clean example
```

## Codes

Each code's entry (what it catches, how, its limits, the fix, and a flagged and a clean example) is in [`lints/`](lints/). `pinoc check --explain <CODE>` prints it and analyses nothing; an unknown code is refused with the list of codes, as `--deny` refuses one. The test suite runs every example through `pinoc check`.

| Code | id | Severity | Confidence | Flags |
|---|---|---|---|---|
| `ACC001-P` | `missing-owner` | deny | likely | An account read as this program's state without an owner check. [More](lints/ACC001-P.md) |
| `ACC002-P` | `missing-signer` | warn | likely | An account compared against a stored authority but never required to sign. [More](lints/ACC002-P.md) |
| `ACC003-P` | `account-confusion` | warn | heuristic | A singleton-named account read as trusted state without its key compared. Hidden at the default threshold. [More](lints/ACC003-P.md) |
| `CPI001-P` | `arbitrary-cpi` | warn | likely | A CPI whose program id comes from an unvalidated caller-supplied account. [More](lints/CPI001-P.md) |
| `ZC001-P` | `layout-padding-mismatch` | warn | likely | A `#[repr(C)]` Shank struct whose padded layout differs from its packed borsh layout. [More](lints/ZC001-P.md) |
| `ZC002-P` | `unchecked-length-before-cast` | deny | likely | Account data borrowed through an `*_unchecked` accessor with no length guard. [More](lints/ZC002-P.md) |
| `ZC003-P` | `missing-repr-c` | deny | definite | A Shank struct read zero-copy without `#[repr(C)]` or `#[repr(transparent)]`. [More](lints/ZC003-P.md) |
| `DISC001-P` | `wrong-discriminator` | deny | definite | A marked discriminator constant that is not the SHA-256 prefix of its preimage. [More](lints/DISC001-P.md) |

**Coverage and meta codes** report what a run could not analyse, so an empty result is never mistaken for a clean one. All three are `warn`/`definite` and appear in every output format. `NO-HANDLERS` and `UNTRACKED-ACCOUNTS` are configured like any other code (`--deny`, `--allow`, `[check]`).

| Code | id | Reports |
|---|---|---|
| `NO-HANDLERS` | `no-handlers` | No handler was found (or there is no `src/`), so the account and CPI lints analysed nothing. Replaces `✅ No issues found.`. [More](lints/NO-HANDLERS.md) |
| `UNTRACKED-ACCOUNTS` | `untracked-accounts` | The handlers whose accounts were not analysed, each with the reason. [More](lints/UNTRACKED-ACCOUNTS.md) |
| `UNUSED-ALLOW` | `unused-allow` | A `// pinoc:allow(CODE)` that matched no finding. Emitted after configuration is applied, so `--deny`/`--allow`/`[check]` do not take it. [More](lints/UNUSED-ALLOW.md) |

`NO-HANDLERS` and `UNTRACKED-ACCOUNTS` describe the whole crate, so they are reported at `src:0:0`. `--deny all` (or `--deny NO-HANDLERS`) makes CI fail on a run that analysed nothing.

**Planned**, listed so the codes are known in advance:

| Code | id | Severity | Flags |
|---|---|---|---|
| `DISC002-P` | `duplicate-discriminator` | deny | Two rows of one marked table, or two marked constants with the same prefix, holding the same bytes. Opt-in through the `DISC001-P` markers. |
| `ACC004-P` | `missing-discriminator` | warn | Two of the crate's state types share a size, one is deserialized, and no tag or discriminator is compared on that path. |
| `ACC005-P` | `duplicate-mutable-account` | warn | Two bindings that both receive lamport or data writes with no `key() != key()` comparison between them, so passing one account twice splits a transfer between "two" accounts that are the same. |
| `CPI003-P` | `revival-on-close` | warn, pending | An account closed by draining lamports without zeroing its data or writing a closed-marker. Pending a check of whether the current runtime still allows same-transaction revival; dropped if it does not. |

**Identifiers.** Every code has two identifiers, both frozen once released:
- the **code** (`ZC001-P`), used in suppression and configuration; the `-P` marks a pinoc code;
- the **id** (`layout-padding-mismatch`), used in prose and output.

**Categories:**
- **ACC** is account validation, including checking an account's discriminator before trusting its data.
- **CPI** is cross-program invocation and account lifecycle.
- **ZC** is zero-copy memory layout.
- **DISC** is discriminator constants: that bytes a program writes are the hash they claim to be.

**Finding properties:**
- a **severity**: `deny` fails the check, `warn` is advisory, and `info` is informational;
- a **confidence**: `heuristic` < `likely` < `definite`;
- a **backend**: `syn` today.

## What gets analysed

**Handlers.** The account and CPI lints run on handlers: any free function, `impl` method or trait default method that takes an accounts slice (`&[AccountView]` or `&[AccountInfo]`, also inside a tuple parameter such as `(data, accounts): (&[u8], &[AccountView])`). Method handlers are named `Type::method`.

**Binding accounts.** Accounts are bound from the slice by:
- a slice pattern, an index, or `.get(i)`/`.first()`/`.last()` (also behind `?`, `ok_or` or `unwrap`);
- a `for` loop over the slice;
- the arms of a `match` on it, including accounts such a `match` hands to a `let`;
- `next_account_info`/`next_account_view`;
- a crate function that is handed the slice and returns an account.

Part of the slice counts as the slice: the halves of `split_at(N)` and the other `split_*` methods, a range such as `accounts[..N]`, and the rest of a slice pattern (`[a, b, rest @ ..]`).

**Accessors are matched by name,** across current and older Pinocchio APIs:
- `owner`/`owned_by`/`is_owned_by`, `is_signer`, `key`/`address`, `data_len`;
- `try_borrow`/`try_borrow_mut`/`try_borrow_data`/`try_borrow_mut_data`;
- the unchecked borrows `borrow_unchecked`/`borrow_unchecked_mut`/`borrow_data_unchecked`/`borrow_mut_data_unchecked`.

A check inside a macro counts, except in logging and formatting macros (`msg!`, `log!`, `println!`, `format!`, ...).

**The whole crate is analysed,** not one function at a time:
- **Helpers are read.** An account passed to a function defined in the crate gets what that function's body does with it, following the helpers it calls in turn. `guards::expect_admin(state, admin)` counts as a signer check only if its body, or a function it calls, tests `is_signer()`.
  - A helper is resolved by its path: `guards::config`, `Config::from_bytes`, `Self::check`, or a trait default method called through an implementor.
  - A method call (`self.check()`, `state.verify(vault)`) is matched against every method of that name in the crate, and counts for what all of them do.
  - **Delegated accounts:** a call that names no single function in the crate, or one outside it, leaves the account delegated, and the lints stay quiet about it rather than guess.
- **Stored accounts are followed.** When a handler moves its accounts into its own type (`Ok(Self { vault, authority })`), they are followed into every function that reaches them through it: `self.vault`, `self.accounts.vault`, `ix.accounts.vault`. A finding on such an account says where it was used and where it was bound.
- **Instructions that share an account list are checked separately.** With `Sell { accounts }` and `Unwind { inner: Sell }`, each is analysed with its own methods, the functions that are no instruction's method, and only those methods of the wrapped type it calls. A check in `Sell::check` does not count for `Unwind` unless `Unwind` calls it. A finding that depends on this names the instruction: ``(in `Sell::pool_reserve`, as part of `Unwind`, bound in `SellAccounts::load`)``.

**Known limits** of the analysis, which works on syntax (the `syn` backend):
- **Source:** only files under `./src` are read.
- **No type resolution:** methods are matched by name and helpers by path. Two functions a path cannot tell apart are treated as unknown, and two same-named methods that differ count only for what they agree on.
- **Unbound accounts:** accounts taken from the slice any other way (`chunks`, `iter().find(..)`, a run-time index) are not bound. A handler that uses its slice and binds nothing from it is listed in `UNTRACKED-ACCOUNTS`; one that binds some accounts and takes others this way is not.
- **Wrapped types:** which methods of a wrapped type an instruction reaches is decided by the names it calls, not by a call graph.
- **Order and paths:** a check counts wherever it appears in the handler or its helpers. pinoc does not prove it runs before the use it guards, or on every path: a helper that checks only under a condition is treated as checking.
- **Owner target:** an owner check counts whichever owner it names.
- **Authority names:** `ACC002-P` recognises an authority by name; add the program's own in `authority_names`.
- **Nested structs:** `ZC001-P` skips a struct with nested or foreign-typed fields.
- **Discriminator tables:** a `DISC001-P` table's enum must be in the same file as the table.

## Configuration

```toml
[check]
deny  = ["ZC001-P"]               # fails the check
warn  = ["ZC003-P"]               # advisory
allow = ["ACC003-P"]              # not reported
confidence_threshold = "likely"   # hide findings weaker than this
authority_names = ["keeper"]      # more names ACC002-P treats as an authority
```

Each key is described in [Configuration](configuration.md#check). An unknown key, a threshold other than `heuristic`, `likely` or `definite`, or an entry that is not a code stops the command, so a typo cannot read as a clean result.

**Precedence.** A flag overrides the file for the codes it names, and within one layer `allow` beats `deny`. The order is: `--allow`, `--deny`, config `allow`, config `deny`, config `warn`, the lint's default.

```bash
pinoc check --deny ZC003-P   # make an advisory code fail
pinoc check --allow ZC001-P  # do not report a code this run (beats a config deny)
pinoc check --deny '*'       # every code fails (quote it so the shell keeps the *)
```

**Wildcards.** `*` or `all` means every code, in any of the lists and in the inline comment. A wildcard matches within its own layer, so `--deny ZC001-P` still overrides a config `allow = ["*"]`.

**Values.** Space-separated values work (`--deny ZC001-P ZC003-P`). A value that is not a code or a wildcard is refused, so a typo, or a bare `--deny *` that the shell expanded into filenames, fails with a clear error.

## Suppression

Inline, by code, on the finding's line or the line directly above, with an optional reason:

```rust
// pinoc:allow(ZC001-P) - padding is intentional, matched by the client
#[repr(C)]
#[derive(ShankAccount)]
pub struct Vault { /* ... */ }
```

- **Where findings are placed:** the struct-layout lints (`ZC001-P`, `ZC003-P`) put their finding at the struct's first attribute, so an allow above the item covers it. The flow lints put it at the offending statement.
- **Wildcard:** `// pinoc:allow(*)` covers any finding there.
- **Stale allows:** an allow that matches nothing is reported as `UNUSED-ALLOW`.

## Output

The exit code is `1` if a surviving finding is `deny`, else `0`, whatever the format. Advisory findings never change it unless denied. `--format` picks the output:

- **`human`** (the default) prints each finding's code, `file:line:col`, evidence and fix.
- **`json`** prints the same findings as a stable array. The JSON shape is frozen. `--json` is the same as `--format json`.
- **`github`** prints one [workflow command](https://docs.github.com/en/actions/reference/workflow-commands-for-github-actions) per finding: `::error` for deny, `::warning` for warn, `::notice` for info. GitHub annotates the file and line on the pull request. The output ends with the human summary lines. GitHub shows at most 10 error and 10 warning annotations per step, and 50 per job; every finding is still in the log.
- **`sarif`** prints [SARIF 2.1.0](https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html) for `github/codeql-action/upload-sarif` (the Security tab) or another code-scanning tool.
  - Every code is a rule, with its `lints/` entry as the help text.
  - Confidence maps to `precision`: `definite` is very-high, `likely` high, `heuristic` medium.
  - No `security-severity` is set until real-program data grounds a number.

**Paths in `github` and `sarif`** start at the repository root, as GitHub needs, even when `pinoc check` runs in a crate below it (`program/src/lib.rs`, not `src/lib.rs`). The root is `GITHUB_WORKSPACE` when set (it is, in Actions), else the nearest directory holding `.git`. Columns are 1-based. `NO-HANDLERS` and `UNTRACKED-ACCOUNTS` go on line 1 of the crate's `Cargo.toml`, or without a location when there is none.

See [CI](ci.md) for workflow recipes.

**The "below the threshold" line.** When `confidence_threshold` hides findings, the human output ends with:

```
N lower-confidence finding(s) (<codes>) below the `<threshold>` threshold hidden; lower `confidence_threshold` (or `--deny <code>`) to show them.
```

- **What it means:** `N` findings were produced but held back because their confidence is below the threshold, such as `ACC003-P` under the default `likely`. They never affect the exit code.
- **To see them:** lower `confidence_threshold` so they print at their own severity, or `--deny <code>` one to force it through, which also makes it fail the check.
- **What it does not count:** findings silenced by `allow`, `--allow` or an inline allow.
- **Other formats:** the threshold applies to them too, so their findings already exclude the hidden ones.

## Discriminator constants

A program that writes Anchor-style discriminators (`sha256("event:Name")[..8]`, ...) as byte literals can mark them for `DISC001-P`:

```rust
// pinoc:discriminators("event:", EventKind)
const DISCRIMINATORS: [[u8; 8]; 25] = [ /* row i is EventKind's variant i */ ];
```

There is a single-constant form and a `snake_case` option; see [DISC001-P](lints/DISC001-P.md). Only marked constants are checked.
