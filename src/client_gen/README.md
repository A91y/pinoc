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

## Codama dependency isolation

Codama's npm dependencies live in a project-local `<out-dir>/.pinoc-codama/`, isolated from the rest of the project. If they are not installed, `pinoc` stops before writing anything (no directory, no `.gitignore` change) and prints the exact `npm install` command rather than installing without consent; pass `--auto-install` to proceed. Rust and TypeScript use different renderer packages, so each language installs its own on first use. `.pinoc-codama/` is added to `.gitignore` automatically when the project is a git repo. Each output directory has its own `.pinoc-codama/` and the two languages use different renderer packages, so generating both a Rust and a TypeScript client installs two `node_modules` trees (about 35 MB each). If Node.js is missing entirely, `pinoc` prints an install pointer.

## Program-derived addresses

When the IDL declares PDAs, the codama Rust client gets `find_program_address` helpers on its accounts. `solana-pubkey` only provides the derivation functions behind its `curve25519` feature, so the generated `Cargo.toml` enables that feature when the client uses them, and leaves the dependency plain otherwise.

## Enums with explicit discriminants

An enum whose variants carry values that are not their positions (`Open = 1, Active = 2, Closed = 5`) is generated with those values by every generator. This needs saying because none of the renderers does it on its own: borsh and `@solana/kit` encode an enum by variant position, and `@codama/renderers-js` and `@codama/renderers-rust` drop the discriminators the IDL carries, so the client would write and read the wrong byte with no error (upstream: [renderers-js#6](https://github.com/codama-idl/renderers-js/issues/6)).

- **`shank`** reads the values from the program source, since the shank IDL does not record them, and emits `Open = 1` with `#[borsh(use_discriminant = true)]`.
- **`codama`** rewrites the rendered enum after the renderer runs: explicit values, plus `#[borsh(use_discriminant = true)]` in Rust or `{ useValuesAsDiscriminators: true }` on the codecs in TypeScript. If the rendered code is not what the rewrite expects, the command fails and says the client must not be used. Output from a renderer that already emits the right values is accepted as it is.

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
