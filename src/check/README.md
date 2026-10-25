# pinoc check

`pinoc check` lints a Pinocchio program for Solana protocol invariants that rustc, clippy, and rust-analyzer do not model: account ownership and signer checks, cross-program invocation safety, and zero-copy memory layout. A program that drains a vault is usually valid Rust that compiles clean; that gap is what this command covers. Issues clippy or rustc already catch (arithmetic overflow, lossy casts, `unwrap`, ignored `Result`) are out of scope.

The command parses every `.rs` file under `src/`, runs each registered lint, applies config and suppression, prints findings, and exits nonzero when a surviving finding is `deny`.

## What gets analysed

The account and CPI lints run on **handlers**: any free function, `impl` method (inherent or trait impl), or trait default method that takes an accounts slice, `&[AccountView]` or `&[AccountInfo]` (also inside a tuple parameter such as `(data, accounts): (&[u8], &[AccountView])`). Accounts are bound from that slice by a slice pattern, by index, by `.get(i)`/`.first()`/`.last()` (also behind `?`, `ok_or` or `unwrap`), by a `for` loop over it, by the arms of a `match` on it (`match accounts { [a, b] => .., _ => .. }`, including the accounts such a `match` hands to a `let`), by `next_account_info`/`next_account_view`, or by a function in the crate that is handed the slice and returns an account. A part of the slice is still the slice: the halves of `accounts.split_at(N)` (and the other `split_*` methods), a range such as `accounts[..N]`, and the rest of a slice pattern (`[a, b, rest @ ..]`) are bound from in the same ways. Method handlers are named `Type::method`.

Accessors are matched by name, covering both current and older Pinocchio APIs: `owner`/`owned_by`/`is_owned_by`, `is_signer`, `key`/`address`, `data_len`, `try_borrow`/`try_borrow_mut`/`try_borrow_data`/`try_borrow_mut_data`, and the unchecked borrows `borrow_unchecked`/`borrow_unchecked_mut`/`borrow_data_unchecked`/`borrow_mut_data_unchecked`.

The analysis covers the whole crate, not one function at a time:

- **Helper functions are read, not skipped.** When an account is passed to a function defined in the crate, `pinoc` applies what that function's body does with it: which checks it makes and how it uses the account, following the helpers it calls in turn. `guards::expect_admin(state, admin)` counts as a signer check only if its body, or a function it calls, tests `is_signer()`. A helper is resolved by its path (`guards::config`, `Config::from_bytes`, `Self::check`, a trait default method called through an implementor); a call that names no single function in the crate, or names one outside it, leaves the account **delegated**, and lints stay quiet about it rather than guess. A method call (`self.check()`, `state.verify(vault)`) has no path, so it is matched against every method of that name in the crate, and counts for what all of them do.
- **Stored accounts are followed.** When a handler moves its accounts into its own type (`Ok(Self { vault, authority })`, the typed-context style), `pinoc` follows them into every function that reaches them through that type: `self.vault` in its methods, `self.accounts.vault` in a struct that holds it, and `ix.accounts.vault` on a local or parameter of either type. A finding on such an account says where it was used and where it was bound.
- **Instructions that share an account list are checked separately.** When two types hold the same accounts type (`Sell { accounts }`, and `Unwind { inner: Sell }` that wraps it), each is analysed as its own instruction: with its own methods, the functions that are no instruction's method, and only those methods of a type it wraps that it calls. A check `Sell::check` makes does not count for `Unwind` unless `Unwind` calls it. A finding that depends on this names the instruction: `(in `Sell::pool_reserve`, as part of `Unwind`, bound in `SellAccounts::load`)`.

What establishes an account, for `ACC001-P`: its owner was checked, its address was compared to an expected one (a constant, a stored key, or a derived PDA), the handler found it empty (`is_data_empty()`), or the handler creates it (`CreateAccount { to: account, .. }`). Any of these means the data read cannot be a look-alike's.

What bounds an unchecked borrow, for `ZC002-P`: a `data_len()` read on the account, a `.len()` test on the borrowed bytes, or handing the bytes to a function that tests their length. Bytes handed directly to a function outside the crate are that function's to bound.

Known limits of the `syn` backend:

- Source is discovered under `./src` only.
- No type resolution: methods are matched by name, and a helper by its path. Two functions that a path cannot tell apart are treated as unknown.
- A method call is matched by name alone. When two methods in the crate share the name and differ in what they do with an account, only what they agree on is applied.
- Accounts taken from the slice any other way (`chunks`, `iter().find(..)`, an index computed at run time) are not bound. A handler that uses its slice and binds nothing from it is listed in `UNTRACKED-ACCOUNTS`; one that binds some accounts and takes others this way is not.
- Which methods of a wrapped type an instruction reaches is decided by the names it calls, not by a call graph.
- A check counts wherever it appears in the handler or its helpers. `pinoc` does not prove it runs before the use it guards, or on every path: a helper that checks only under a condition is treated as checking.
- An owner check counts whichever owner it names. `owned_by(&TOKEN_PROGRAM_ID)` satisfies `ACC001-P` like `owned_by(&crate::ID)`.
- Accounts stored in a type that nothing in the crate reads back through are reported as `UNTRACKED-ACCOUNTS`.
- `ACC002-P` recognises an authority by the name it is compared against: `authority`, `admin`, `auth`, and names ending in `_authority` or `_auth`. Add the program's own in `Pinoc.toml` (`authority_names`).
- `ZC001-P` skips a struct with nested or foreign-typed fields.

### Coverage findings

Two codes report what a run could not analyse, so an empty result is never mistaken for a clean one. They are `warn`/`definite`, appear in `--json`, and are configured like any other code (`--deny`, `--allow`, `Pinoc.toml [check]`). Both describe the whole project, so they are reported at `src:0:0` and not at a source line.

| Code | id | Reports |
|---|---|---|
| `NO-HANDLERS` | `no-handlers` | No handler was found anywhere (or there is no `src/`), so the account and CPI lints analysed nothing. Replaces `✅ No issues found.`. [More](../../docs/lints/NO-HANDLERS.md) |
| `UNTRACKED-ACCOUNTS` | `untracked-accounts` | The handlers whose accounts were not analysed, each with the reason. Checks and uses in them are not reported. [More](../../docs/lints/UNTRACKED-ACCOUNTS.md) |
| `UNUSED-ALLOW` | `unused-allow` | A `// pinoc:allow(CODE)` that matched no finding. Emitted after configuration is applied, so `--deny`/`--allow`/`[check]` do not take it. [More](../../docs/lints/UNUSED-ALLOW.md) |

`--deny all` (or `--deny NO-HANDLERS`) makes CI fail on a run that analysed nothing.

## Lint codes

Every check has two stable identifiers, frozen once released:

- **code** (`ZC001-P`) - canonical in suppression and config. The trailing `-P` marks it a pinoc code. Once shipped it is never renamed, so `// pinoc:allow(ZC001-P)` keeps working.
- **id** (`layout-padding-mismatch`) - the readable name, canonical in prose and output.

Codes are namespaced by category:

- **ACC** - account validation (owner, signer, key, account discriminator check).
- **CPI** - cross-program invocation and account lifecycle.
- **ZC** - zero-copy memory layout.
- **DISC** - discriminator constants: that the bytes a program writes are the hash they claim to be. Checking an account's discriminator at run time before trusting its data is account validation, and belongs in ACC.

Each finding also carries a **severity** (`deny` fails the check, `warn` is advisory, `info`), a **confidence** (`heuristic` < `likely` < `definite`), and a backend (`syn` today).

## Current checks

Each code's full entry (what it catches, how it is detected, its limits, the fix, and a flagged and a clean example) is in [`docs/lints/`](../../docs/lints/), and `pinoc check --explain <CODE>` prints it. The examples are run through `pinoc check` by the test suite.

| Code | id | Severity | Confidence | Flags |
|---|---|---|---|---|
| `ACC001-P` | `missing-owner` | deny | likely | An account read as this program's state without an owner check. [More](../../docs/lints/ACC001-P.md) |
| `ACC002-P` | `missing-signer` | warn | likely | An account compared against a stored authority but never required to sign. [More](../../docs/lints/ACC002-P.md) |
| `ACC003-P` | `account-confusion` | warn | heuristic | A singleton-named account read as trusted state without its key compared. Hidden at the default `likely` threshold. [More](../../docs/lints/ACC003-P.md) |
| `CPI001-P` | `arbitrary-cpi` | warn | likely | A CPI whose program id comes from an unvalidated caller-supplied account. [More](../../docs/lints/CPI001-P.md) |
| `ZC001-P` | `layout-padding-mismatch` | warn | likely | A `#[repr(C)]` Shank struct whose padded layout differs from its packed borsh layout. [More](../../docs/lints/ZC001-P.md) |
| `ZC002-P` | `unchecked-length-before-cast` | deny | likely | Account data borrowed through an `*_unchecked` accessor with no length guard. [More](../../docs/lints/ZC002-P.md) |
| `ZC003-P` | `missing-repr-c` | deny | definite | A Shank struct read zero-copy without `#[repr(C)]` or `#[repr(transparent)]`. [More](../../docs/lints/ZC003-P.md) |
| `DISC001-P` | `wrong-discriminator` | deny | definite | A marked discriminator constant that is not the SHA-256 prefix of its preimage. [More](../../docs/lints/DISC001-P.md) |

The struct-layout lints (`ZC001-P`, `ZC003-P`) anchor their finding at the struct's first attribute, so a suppression comment written directly above the item covers it. The flow lints (`ACC001-P`, `ACC002-P`, `ACC003-P`, `ZC002-P`, `CPI001-P`) anchor at the offending statement.

### Discriminator constants

A program that writes Anchor-style discriminators (`sha256("event:Name")[..8]`, `sha256("account:Name")[..8]`, `sha256("global:snake_name")[..8]`) as byte literals can mark them, and `DISC001-P` recomputes each one. A marker goes in a `//` comment directly above the `const`, or among its doc comments and attributes:

```rust
// pinoc:discriminator("account:Vault")
pub const VAULT_DISCRIMINATOR: [u8; 8] = [211, 8, 232, 43, 2, 152, 117, 119];

pub enum EventKind { VaultDeposit = 0, WithdrawRequested = 1, /* .. */ }

// pinoc:discriminators("event:", EventKind)
const DISCRIMINATORS: [[u8; 8]; 25] = [
    [4, 248, 234, 163, 99, 238, 140, 45],   // VaultDeposit
    [114, 16, 240, 206, 93, 128, 151, 39],  // WithdrawRequested
    // ..
];
```

In the table form, row `i` is checked against the variant of the enum whose discriminant is `i` (the declared value, or one more than the previous variant's), so a variant added, removed or reordered without its row is reported. A third argument, `snake_case`, hashes the variant name in snake case (`// pinoc:discriminators("global:", Instruction, snake_case)`). The discriminator length is the row's own (1 to 32 bytes). The finding is anchored at the wrong row, and its fix gives the right bytes.

A marker that cannot be checked is reported, never skipped, with the reason: the enum is not in the file, the table has more or fewer rows than the enum has variants, a variant's discriminant is not an integer literal, no variant has a row's index, or a value is not a byte array literal (a row written as another const's name, say). `pinoc:discriminator` and `pinoc:discriminators` are told apart by the whole name, so neither form is read as the other.

Only markers are checked: a crate with none gets no `DISC001-P` finding, and its `pinoc check` output is unchanged.

**Same-file limit.** The table's enum has to be in the same file as the table. The lint runs per file (`Lint::run`), and resolving an enum by name across modules would be ambiguous when two modules define one with that name, so the first version keeps to the file and reports an enum it cannot find there. A crate-level pass (`Lint::run_handlers` works over facts from the whole crate) could lift this later, with a rule for the ambiguous case.

## Planned checks

Not yet implemented; codes and intended severity are listed so the suppression contract is known in advance.

| Code | id | Category | Severity | Flags |
|---|---|---|---|---|
| `DISC002-P` | `duplicate-discriminator` | DISC | deny | Two rows of one marked table, or two marked constants with the same prefix, holding the same bytes. Opt-in through the `DISC001-P` markers. |
| `ACC004-P` | `missing-discriminator` | ACC | warn | Two or more of the crate's state types share a size, one is deserialized, and nothing on that path reads and compares a tag byte or discriminator, so one account type can be read as another. |
| `ACC005-P` | `duplicate-mutable-account` | ACC | warn | Two account bindings that both receive lamport or data writes with no `key() != key()` comparison between them, so passing one account twice splits a transfer between "two" accounts that are the same. |
| `CPI003-P` | `revival-on-close` | CPI | warn, pending | An account closed by draining lamports without zeroing its data or writing a closed-marker, allowing same-transaction revival. Pending a check of what the current runtime does with an account drained to zero lamports within the transaction; dropped if revival is no longer possible. |

Out of scope: a missing writable check. The runtime rejects a write to an account the transaction did not mark writable, so there is nothing for a linter to add.

## Configuration

`Pinoc.toml`:

```toml
[check]
deny  = ["ZC001-P"]          # promote to deny (fails the check)
warn  = ["ZC003-P"]          # downgrade to advisory
allow = ["ACC003-P"]         # suppress entirely
confidence_threshold = "likely"   # drop findings weaker than this
authority_names = ["keeper"]      # more names ACC002-P treats as an authority that must sign
```

A key under `[check]` that is not one of these, or a section `pinoc` does not know, stops the command with the name it did not recognise. So does a value that cannot be applied: a `confidence_threshold` other than `heuristic`, `likely` or `definite`, or an entry in `deny`, `warn` or `allow` that is not a lint code. A misspelt `authority_names` would otherwise leave a finding unreported, which reads the same as a clean program.

**Precedence.** A CLI flag overrides the config file for the code(s) it names: `--deny ZC001-P` wins over a config `allow = ["ZC001-P"]` (and over a config `allow = ["*"]`), and `--allow` beats a config `deny`. Within one layer, `allow` beats `deny`. So the order is: CLI allow, then CLI deny, then config allow, then config deny, then config warn, then the lint's default.

```bash
pinoc check --deny ZC003-P   # make an advisory code fail
pinoc check --allow ZC001-P  # suppress a code for this run (beats a config deny)
pinoc check --deny '*'       # every code fails (quote so the shell keeps the *)
```

`*` or `all` targets every code in the deny, warn, and allow lists (`*` also works in the inline comment). A wildcard matches within its own layer, so a specific `--deny ZC001-P` still overrides a config `allow = ["*"]`. Space-separated values work too (`--deny ZC001-P ZC003-P`). Any value that is not a real code or `*`/`all` is rejected, so a typo or a bare `--deny *` (which the shell expands into filenames) fails with a clear error; quote it as `--deny '*'` or use `--deny all`.

## Suppression

Inline, by code, on the line directly above the item (reason optional):

```rust
// pinoc:allow(ZC001-P) - padding is intentional, matched by the client
#[repr(C)]
#[derive(ShankAccount)]
pub struct Vault { /* ... */ }
```

`// pinoc:allow(*)` suppresses any finding on that item. An allow that matches nothing is reported as an `UNUSED-ALLOW` warning.

## Exit code and output

The process exits `1` if any surviving finding is `deny`, else `0`, whatever the output format. Advisory findings never change the exit code unless explicitly denied.

`--format` picks the output:

- **`human`** (the default) groups findings with their code, `file:line:col`, evidence, and fix.
- **`json`** emits the same findings as a stable array. The JSON shape is frozen. `--json` is the same as `--format json`.
- **`github`** prints one [workflow command](https://docs.github.com/en/actions/reference/workflow-commands-for-github-actions) per finding: `::error` for deny, `::warning` for warn, `::notice` for info. GitHub then annotates the file and line on the pull request. It ends with the human summary lines.
- **`sarif`** emits [SARIF 2.1.0](https://docs.oasis-open.org/sarif/sarif/v2.1.0/sarif-v2.1.0.html) for `github/codeql-action/upload-sarif` (the Security tab) or another code-scanning tool.
  - Every code is listed as a rule, with its `docs/lints` entry as the help text, and its confidence as `precision` (`definite` very-high, `likely` high, `heuristic` medium).
  - No `security-severity` is set: a number per lint would be invented before there is real-program data to ground it.

**Paths:** `github` and `sarif` give paths from the repository root, as GitHub needs, even when `pinoc check` runs in a crate below it (`program/src/lib.rs`, not `src/lib.rs`). The root is `GITHUB_WORKSPACE` when it is set (it is, in Actions), else the nearest directory holding `.git`.

**Placement:** columns are 1-based there. The project-wide findings (`NO-HANDLERS`, `UNTRACKED-ACCOUNTS`), reported at `src:0:0` elsewhere, go on line 1 of the crate's `Cargo.toml`, or without a location when there is none.

**Annotation limits:** GitHub shows at most 10 error and 10 warning annotations per step, and 50 per job. Every finding is still printed in the step's log.

```yaml
# Fail the job on a deny, and annotate the diff.
- uses: a91y/pinoc@v0.4.0
  with: { version: 0.4.0 }
- run: pinoc check --format github
  working-directory: program

# Or send the findings to the Security tab. `|| true` lets the upload run when
# the check fails, so this step is not the gate.
- run: pinoc check --format sarif > pinoc.sarif || true
  working-directory: program
- uses: github/codeql-action/upload-sarif@v3
  with: { sarif_file: program/pinoc.sarif }
```

`pinoc check --explain <CODE>` prints a code's `docs/lints` entry and analyses nothing. An unknown code is refused with the list of codes, as `--deny` refuses one.

### The "below the threshold" line

When `confidence_threshold` drops one or more findings for being too low-confidence (see [Configuration](#configuration)), the human output prints a trailing line so the hidden count is never silent:

```
N lower-confidence finding(s) (<codes>) below the `<threshold>` threshold hidden; lower `confidence_threshold` (or `--deny <code>`) to show them.
```

It means exactly `N` findings *were produced* but held back because their confidence is weaker than the active `confidence_threshold` (for example, a `heuristic` lint like `ACC003-P` under the default `likely` threshold). They are not failures and never affect the exit code. To see them, lower `confidence_threshold` (e.g. to `heuristic`) so they print at their natural severity, or `--deny <code>` a specific one to force it through (which also makes it fail the check). The count only includes findings hidden by the threshold, not ones you silenced with `allow`/`--allow` or an inline `// pinoc:allow`. This line appears in human output only; the threshold applies to `--json` too, so its array already excludes the hidden findings (lower `confidence_threshold` to include them).

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
| `../../docs/lints/<CODE>.md` | One entry per code: what it catches, how, its limits, the fix, and a flagged and a clean example. The test suite runs the examples through `pinoc check`. |
