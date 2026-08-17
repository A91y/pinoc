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
| `ACC002-P` | `missing-signer` | warn | likely | An account whose key is checked against a stored authority field (`authority.address() == state.authority`, via `==`/`!=` or `.eq()`/`.ne()`) but never `is_signer()`-checked, so anyone can act as that authority by passing its public address. Runs on the fact table; a delegated account is left alone. `warn` because the syn v1 cannot see a signer enforced in a helper or downstream token CPI. Fix: require `is_signer()` on the authority. |
| `ACC003-P` | `account-confusion` | warn | heuristic | An account named like a known singleton (`config`/`settings`/`*_config`/`*_settings`) read as trusted state without its key ever being compared to an expected address, letting an attacker substitute a look-alike account with attacker-chosen contents. Runs on the fact table; a delegated account is left alone. `heuristic`, so it is **hidden at the default `likely` threshold** (see the note below); surface it with `--deny ACC003-P` or `confidence_threshold = "heuristic"`. Fix: compare the account's key to the expected address (or derived PDA) before reading it. |
| `CPI001-P` | `arbitrary-cpi` | warn | likely | `invoke`/`invoke_signed` whose `program_id` comes from a caller-supplied account whose key is never compared to an expected id (`==`/`!=` or an assert/require macro), letting an attacker redirect the call to malicious code. Runs on the fact table: typed-builder CPIs (`CreateAccount { … }.invoke_signed(…)`) and constant/param program ids are ignored, and a delegated program account is left alone. `warn` because the syn v1 cannot see cross-function key checks; promote with `--deny CPI001-P`. Fix: compare the program account's key to the expected id before invoking. |
| `ZC001-P` | `layout-padding-mismatch` | warn | likely | A `#[repr(C)]` `ShankAccount`/`ShankType` struct whose padded in-memory layout differs from its packed borsh layout, so on-chain zero-copy reads and client (de)serialization disagree. Advisory because it only affects the generated borsh client; a program driven another way is unaffected. Reuses the layout analysis behind the `pinoc build`/`pinoc idl` padding warning. Fix: add explicit `_padding: [u8; N]` fields, or `--deny ZC001-P` to make it blocking. |
| `ZC002-P` | `unchecked-length-before-cast` | deny | likely | An account whose data is borrowed through an `*_unchecked` accessor (`borrow_data_unchecked`/`borrow_mut_data_unchecked`) with no `data_len()` guard on that account, so a shorter-than-expected account is read past its end. Runs on the per-account fact table (`facts/`); if the account's `data_len()` is read anywhere in the handler it is treated as guarded, and a delegated account is left alone. Fix: check `data_len() >= size_of::<T>()` before the unchecked borrow. |
| `ZC003-P` | `missing-repr-c` | deny | definite | A `ShankAccount`/`ShankType` struct read zero-copy without `#[repr(C)]` or `#[repr(transparent)]`. The default layout may reorder fields and break the byte mapping the client relies on. Fix: add `#[repr(C)]`. |

The struct-layout lints (`ZC001-P`, `ZC003-P`) anchor their finding at the struct's first attribute, so a suppression comment written directly above the item covers it. The flow lints (`ACC001-P`, `ACC002-P`, `ACC003-P`, `ZC002-P`, `CPI001-P`) anchor at the offending statement.

<details>
<summary><strong>Note on <code>ACC002-P</code>: name-match detection</strong></summary>

`ACC002-P` decides an account is "in an authority position" by a **string/name match**: it fires only when the account's key is compared against a field or variable whose name is `authority`, `admin`, `auth`, or ends in `_authority`/`_auth`. This is a deliberate precision-over-recall choice. When it matches, it is very likely a real missing-signer, but it **misses** any authority stored under a different name (`controller`, `owner_key`, `gov`, …), a pure false negative.

The naming-agnostic improvement is to key off **structure** instead: treat "the account's key is compared against a field of a state account that was `load`ed" as the authority signal, regardless of the field's name. The reason we do not do that yet is that it **overfires**: comparing a key against *any* loaded-state field (a stored `mint`, a `bump`, a config value) would then look like an authority check and produce false positives. Doing it safely means also gating on "a privileged effect actually happens in the handler," and the fully precise form (proving a signer check dominates the privileged action, seeing cross-function checks, and resolving PDA-seeds / token-CPI enforcement) needs the type-resolved dylint backend (Phase 5). Until then the name match is kept as-is: fewer, higher-confidence findings at `warn`, promotable with `--deny ACC002-P`.

</details>

<details>
<summary><strong>Note on <code>ACC003-P</code>: why it is `heuristic` for now</strong></summary>

`ACC003-P` fires when an account whose *name* looks like a global singleton (`config`, `settings`, `*_config`, `*_settings`) is read as trusted state but its key is never compared to an expected address. When the account really is a singleton, that is a genuine account-confusion bug. The problem is the engine cannot tell a **singleton** (there is exactly one, so its key must equal a fixed or derived known address) from a **per-instance** account (there are many, one per caller/pool/position, whose key legitimately varies).

This is not just a per-user issue. In a DeFi program the same shape recurs at several granularities: a **per-pool** config, a **per-position** state, a **per-market** account. Each of those *should* be validated, but by **deriving its PDA and comparing** (from the pool id, position id, owner, …), not by matching a single constant. To a name/shape heuristic, `pool_config` or `position_state` looks identical to a global `config`, so a syn-only pass would over-fire on the per-instance ones. That is why it ships `heuristic` and stays **hidden at the default `likely` threshold**: available to anyone who opts in (`confidence_threshold = "heuristic"` or `--deny ACC003-P`), but not nagging by default.

The move to `likely` comes with the dylint backend (Phase 5), which can **resolve the PDA derivation**: constant seeds mean a singleton (must equal a fixed address), while seeds that include a variable (owner, pool id, position id) mean a per-instance account (must be derived-and-compared, and a `KeyCompared` against that derived value already clears it). Once singleton and per-instance can be told apart from the seeds, the over-firing disappears and the lint can be promoted.

</details>

## Planned checks

Not yet implemented; codes and intended severity are listed so the suppression contract is known in advance.

| Code | id | Category | Severity | Flags |
|---|---|---|---|---|
| `ACC004-P` | `missing-discriminator` | ACC | warn | Deserialize of an equal-size type (cross-referencing the ZC001 size table) with no discriminator check, so one account type is read as another. |
| `ACC005-P` | `duplicate-mutable-account` | ACC | warn | Two mutably-used account bindings with no `key() != key()` guard between them. |
| `CPI003-P` | `revival-on-close` | CPI | deny | An account closed by draining lamports without zeroing its data or writing a closed-marker, allowing same-transaction revival. |

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
| `facts/mod.rs` | Per-account fact table: for each handler, how each account is validated and used, plus the program-id source of each `invoke`. Account/CPI lints run on this. |
| `lints/mod.rs` | The lint registry and span helpers. |
| `lints/acc001_owner.rs`, `lints/acc002_signer.rs`, `lints/acc003_confusion.rs`, `lints/cpi001_arbitrary_cpi.rs`, `lints/zc001_padding.rs`, `lints/zc002_length.rs`, `lints/zc003_repr_c.rs` | The individual checks. |
