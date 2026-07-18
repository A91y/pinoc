# pinoc check

`pinoc check` lints a Pinocchio program for Solana protocol invariants that rustc, clippy, and rust-analyzer do not model: account ownership and signer checks, cross-program invocation safety, and zero-copy memory layout. A program that drains a vault is usually valid Rust that compiles clean; that gap is what this command covers. Issues clippy or rustc already catch (arithmetic overflow, lossy casts, `unwrap`, ignored `Result`) are out of scope.

The command parses every `.rs` file under `src/`, runs each registered lint, applies config and suppression, prints findings, and exits nonzero when a surviving finding is `deny`.

## Lint codes

Every check has two stable identifiers, frozen once released:

- **code** (`ZC001-P`) - canonical in suppression and config. The trailing `-P` marks it a pinoc code. Once shipped it is never renamed, so `// pinoc:allow(ZC001-P)` keeps working.
- **id** (`layout-padding-mismatch`) - the readable name, canonical in prose and output.

Codes are namespaced by category:

- **ACC** - account validation (owner, signer, key, discriminator).
- **CPI** - cross-program invocation and account lifecycle.
- **ZC** - zero-copy memory layout.

Each finding also carries a **severity** (`deny` fails the check, `warn` is advisory, `info`), a **confidence** (`heuristic` < `likely` < `definite`), and a backend (`syn` today).

## Current checks

| Code | id | Severity | Confidence | Flags |
|---|---|---|---|---|
| `ACC001-P` | `missing-owner` | deny | likely | An account read as this program's state (data borrowed, or passed to a loader like `Type::load`/`from_bytes`) without checking `owner() == program_id`, letting an attacker pass a look-alike account. Runs on the per-account fact table (`facts/`); an account passed to a function the analyzer cannot see into is treated as delegated and left alone. Fix: check the owner before reading the account's data. |
| `ZC001-P` | `layout-padding-mismatch` | warn | likely | A `#[repr(C)]` `ShankAccount`/`ShankType` struct whose padded in-memory layout differs from its packed borsh layout, so on-chain zero-copy reads and client (de)serialization disagree. Advisory because it only affects the generated borsh client; a program driven another way is unaffected. Reuses the layout analysis behind the `pinoc build`/`pinoc idl` padding warning. Fix: add explicit `_padding: [u8; N]` fields, or `--deny ZC001-P` to make it blocking. |
| `ZC003-P` | `missing-repr-c` | deny | definite | A `ShankAccount`/`ShankType` struct read zero-copy without `#[repr(C)]` or `#[repr(transparent)]`. The default layout may reorder fields and break the byte mapping the client relies on. Fix: add `#[repr(C)]`. |

Both anchor their finding at the struct's first attribute, so a suppression comment written directly above the item covers it.

## Planned checks

Not yet implemented; codes and intended severity are listed so the suppression contract is known in advance.

| Code | id | Category | Severity | Flags |
|---|---|---|---|---|
| `CPI001-P` | `arbitrary-cpi` | CPI | deny | `invoke`/`invoke_signed` to a caller-supplied program account never checked against an expected id, letting an attacker redirect the call to malicious code. |
| `ZC002-P` | `unchecked-length-before-cast` | ZC | deny | Account data cast to a type (`from_bytes`, pointer cast, `borrow_data_unchecked`) without a preceding `data_len() >= size_of::<T>()` guard, a buffer over-read. |

## Configuration

`Pinoc.toml`:

```toml
[check]
deny  = ["ZC001-P"]          # promote to deny (fails the check)
warn  = ["ZC003-P"]          # downgrade to advisory
allow = ["ACC003-P"]         # suppress entirely
confidence_threshold = "likely"   # drop findings weaker than this
```

CLI flags layer on top of the file, and `--allow`/`allow` win over `--deny`/`deny`:

```bash
pinoc check --deny ZC003-P   # make an advisory code fail
pinoc check --allow ZC001-P  # suppress a code for this run
pinoc check --deny '*'       # every code fails (quote so the shell keeps the *)
```

`*` or `all` targets every code in the deny, warn, and allow lists (`*` also works in the inline comment). Space-separated values work too (`--deny ZC001-P ZC003-P`). Any value that is not a real code or `*`/`all` is rejected, so a typo or a bare `--deny *` (which the shell expands into filenames) fails with a clear error; quote it as `--deny '*'` or use `--deny all`.

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

The process exits `1` if any surviving finding is `deny`, else `0`. Advisory findings never change the exit code unless explicitly denied. Human output groups findings with their code, `file:line:col`, evidence, and fix; `--json` emits the same findings as a stable array (the JSON shape is frozen).

## Module map

| Path | Responsibility |
| --- | --- |
| `mod.rs` | Discovers and parses source, runs lints, applies config severity, suppression, and the confidence threshold, sets the exit code. |
| `contract.rs` | `Finding`, the `Lint` trait, and `Severity` / `Confidence` / `Category` / `Backend` / `Span`. The JSON shape is frozen here. |
| `suppress.rs` | Parses `// pinoc:allow(CODE)` comments and matches them to findings. |
| `output.rs` | Human and `--json` renderers. |
| `facts/mod.rs` | Per-account fact table: for each handler, how each account is validated and used. Account/CPI lints run on this. |
| `lints/mod.rs` | The lint registry and span helpers. |
| `lints/acc001_owner.rs`, `lints/zc001_padding.rs`, `lints/zc003_repr_c.rs` | The individual checks. |
