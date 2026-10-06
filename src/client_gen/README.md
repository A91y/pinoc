# Client generation

`pinoc client generate` renders a standalone Rust client crate from the IDL, or a TypeScript client with `--language ts`. Run `pinoc build` (or `pinoc idl`) first so the IDL exists. The generated crate is self-contained: `cd clients/rust-shank && cargo build` (or `clients/rust-codama`); it is not part of the program's Cargo workspace.

Two generators are available.

- **`shank`**: pure Rust, built into `pinoc`, no Node.js required. Reads `target/idl/<name>.json`. Emits borsh instruction builders, account (de)serialization, defined types, `declare_id!` matching the program, and (optionally) CPI variants and `fetch_*` RPC helpers. Its output mirrors the conventions of Codama's Rust renderer, though it is not that renderer. Type coverage: primitives, arrays, `Vec`, `Option`, byte vectors, pubkeys, and struct and enum defined types. Not yet rendered by shank (each fails with a clear message): tuples, maps, and sets (render these with `--generator codama` instead), plus bytemuck `PodOption` and nested account groups. Note that `PodOption` (shank's fixed-size option) also fails on the codama path for a shank program, because the shim feeds `@codama/nodes-from-anchor`, which does not recognize shank's extension; render it with Codama's native macros instead.
- **`codama`**: shells out to the real [Codama](https://github.com/codama-idl/codama) JS pipeline (`@codama/nodes-from-anchor` + `@codama/renderers-rust`). Reads `target/idl/<name>.codama.json`. Requires Node.js/npm.

```bash
pinoc client generate                                     # prompts for shank/codama if interactive
pinoc client generate --generator shank
pinoc client generate --generator codama --auto-install   # install codama's npm deps on first run
```

## TypeScript

`--language ts` renders a TypeScript client through the codama generator ([`@codama/renderers-js`](https://github.com/codama-idl/renderers-js)); the default is `--language rust`. It writes `clients/ts/` (`src/generated/`, a `src/index.ts` re-export, and a `package.json` with the `@solana/kit` dependencies), next to the Rust clients. `--language ts` selects codama on its own, with no prompt or confirmation; combining it with `--generator shank` is refused, because the built-in generator only renders Rust.

```bash
pinoc client generate --language ts --auto-install
```

The `[client]` paths in `Pinoc.toml` configure the Rust clients only; pass `--out-dir` to move the TypeScript one.

The recommended generator (shank, or codama when Codama macros are detected) is the default when picking interactively. Passing `--generator` against the recommendation asks for confirmation; skip it with `-y`. Non-interactively without `-y`, it refuses rather than guess.

## CPI variants

`pinoc client generate` scans the program source for `invoke`/`invoke_signed` call sites and, when found, emits `XxxCpi` / `XxxCpiAccounts` / `XxxCpiBuilder` for each instruction (to call this program from another program), plus the three dependencies they need (`solana-account-info`, `solana-cpi`, `solana-program-error`). These helpers consume `solana_account_info::AccountInfo`, so the calling program must be solana-program-style, not pinocchio.

```bash
pinoc client generate --with-cpi   # force CPI variants even if not detected
pinoc client generate --no-cpi     # never generate them
```

## `fetch_*` RPC helpers

`fetch_x` / `fetch_all_x` / `fetch_maybe_x` / `fetch_all_maybe_x` are generated per account, gated behind a `fetch` Cargo feature so `solana-rpc-client` / `solana-account` are only pulled in when opted into: `cargo build --features fetch` in the generated crate.

## Output paths

Default output depends on the resolved generator (`clients/rust-shank` or `clients/rust-codama`), so running both does not overwrite one with the other. Override precedence:

`--out-dir` (CLI) > `shank_out_dir` / `codama_out_dir` > `out_dir` (shared) > the per-generator default.

```toml
# Pinoc.toml
[client]
out_dir = "clients/rust"          # shared; warns each run, since switching generators overwrites the other's output
shank_out_dir = "clients/shank"   # per-generator, wins over the shared out_dir
codama_out_dir = "clients/codama"
```

## Codama dependencies

Codama's npm dependencies live in a project-local `<out-dir>/.pinoc-codama/`, isolated from the rest of the project. `pinoc` installs them only with consent: without `--auto-install` it stops before writing anything (no directory, no `.gitignore` change) and says so. `.pinoc-codama/` is added to `.gitignore` automatically when the project is a git repo. If Node.js is missing entirely, `pinoc` prints an install pointer.

**The versions are fixed by pinoc, not by npm.** Each pinoc release names exact versions of the packages it drives and carries a lock file for their whole dependency tree, and installs with `npm ci`, which installs exactly that tree or fails. The same pinoc therefore installs the same packages on any machine and any day, and a renderer upgrade is a pinoc change:

| | Rust client | TypeScript client |
|---|---|---|
| `codama` | 1.11.0 | 1.11.0 |
| `@codama/nodes-from-anchor` | 1.5.6 | 1.5.6 |
| renderer | `@codama/renderers-rust` 1.2.10 | `@codama/renderers-js` 2.5.0 |

Every run prints the three versions it rendered with, and checks the installed tree before using it:

- **It came from this pinoc's lock.** `pinoc` records the lock an install came from (`.pinoc-codama/installed-lock.json`). A tree installed by another pinoc version, or by a hand-run `npm install`, is not used. Upgrading pinoc across a lock change therefore needs `--auto-install` once.
- **It has not changed since.** `npm ci` verifies the lock's integrity hashes only while installing, so `pinoc` also records a fingerprint of every file it installed (`.pinoc-codama/installed-tree`) and compares it on each run. An edited, added or removed file, or a cache that restored a different tree, is refused. This notices a change; it is not a defence against someone who rewrites the fingerprint too.

Either way the command stops before rendering and asks for `--auto-install`, which reinstalls. A hand-run `npm ci` inside `.pinoc-codama/` restores the same tree and is accepted.

Each output directory has its own `.pinoc-codama/` and the two languages use different renderer packages, so generating both a Rust and a TypeScript client installs two `node_modules` trees (about 35 MB each).

## Program-derived addresses

When the IDL declares PDAs, the codama Rust client gets `find_program_address` helpers on its accounts. `solana-pubkey` only provides the derivation functions behind its `curve25519` feature, so the generated `Cargo.toml` enables that feature when the client uses them, and leaves the dependency plain otherwise. Without it the client fails to build with `E0599: no function or associated item named create_program_address found for struct Pubkey`, which does not mention features; that was the case for clients generated by pinoc 0.3.2 and earlier.

## Enums with explicit discriminants

An enum whose variants carry values that are not their positions (`Open = 1, Active = 2, Closed = 5`) is generated with those values by every generator. This needs saying because none of the renderers does it on its own: borsh and `@solana/kit` encode an enum by variant position, and `@codama/renderers-js` and `@codama/renderers-rust` drop the discriminators the IDL carries, so the client would write and read the wrong byte with no error (upstream: [renderers-js#6](https://github.com/codama-idl/renderers-js/issues/6)).

- **`shank`** reads the values from the program source, since the shank IDL does not record them, and emits `Open = 1` with `#[borsh(use_discriminant = true)]`.
- **`codama`** rewrites the rendered enum after the renderer runs: explicit values, plus `#[borsh(use_discriminant = true)]` in Rust or `{ useValuesAsDiscriminators: true }` on the codecs in TypeScript. If the rendered code is not what the rewrite expects, the command fails and says the client must not be used; since the renderer versions are fixed by pinoc, that can only follow a pinoc upgrade. Output from a renderer that already emits the right values is accepted as it is.

`pinoc client generate` prints which enums this applied to. Two shapes are refused, because they cannot be corrected: explicit non-positional discriminators on an enum whose variants carry data, and on an enum declared inline instead of as its own type. An enum numbered `0..n` is generated exactly as before.

## Zero-copy layout

The generated client (de)serializes as packed borsh, while scaffolded programs read zero-copy via `#[repr(C)]`. Keep `#[repr(C)]` IDL structs padding-free so the two agree; see [../idl/README.md](../idl/README.md#zero-copy-padding-lint).

## Module map

| Path | Responsibility |
| --- | --- |
| `mod.rs` | Dispatches to the `shank` or `codama` generator. |
| `shank/mod.rs` | Orchestrates the built-in Rust renderer. |
| `shank/instructions.rs` | Instruction builders. |
| `shank/accounts.rs` | Account (de)serialization and `fetch_*` helpers. |
| `shank/types.rs` | Defined types. |
| `shank/cpi.rs` | CPI variants (`XxxCpi` / `XxxCpiBuilder`). |
| `shank/manifest.rs` | The generated crate's `Cargo.toml`. |
| `shank/shared.rs` | Shared render helpers. |
| `discriminants.rs` | Finds enums with explicit, non-positional discriminants and makes every generated client use them. |
| `codama/mod.rs` | Drives the external Codama JS pipeline (Rust and TypeScript renderers). |
