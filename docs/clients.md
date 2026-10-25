# Client generation

`pinoc client generate` renders a client from the [IDL](idl.md): a standalone Rust crate, or a TypeScript package with `--language ts`. Run `pinoc build` or `pinoc idl` first so the IDL exists. The Rust crate is self-contained (`cd clients/rust-shank && cargo build`) and is not part of the program's Cargo workspace.

## Generators

- **`shank`** is pure Rust, built into pinoc, with no Node.js. It reads `<name>.json` and emits:
  - borsh instruction builders;
  - account (de)serialization;
  - defined types;
  - `declare_id!` matching the program;
  - optionally, [CPI variants](#cpi-variants) and [`fetch_*` helpers](#fetch_-rpc-helpers).

  Its output follows the conventions of Codama's Rust renderer, though it is not that renderer.
  - **Types it renders:** primitives, arrays, `Vec`, `Option`, byte vectors, pubkeys, and struct and enum defined types.
  - **Not yet rendered** (each fails with a clear message): tuples, maps and sets (use `--generator codama` for those), bytemuck `PodOption`, and nested account groups.
  - **`PodOption` on the codama path:** it also fails there for a shank program, because the shim feeds `@codama/nodes-from-anchor`, which does not know shank's extension. Use Codama's native macros for it.
- **`codama`** runs the real [Codama](https://github.com/codama-idl/codama) JS pipeline (`@codama/nodes-from-anchor` and `@codama/renderers-rust`) on `<name>.codama.json`. It needs Node.js and npm.

```bash
pinoc client generate                                     # prompts for shank/codama if interactive
pinoc client generate --generator shank
pinoc client generate --generator codama --auto-install   # installs codama's npm deps on first run
```

**Choosing a generator:**
- **Without `--generator`:** pinoc uses `[client] generator` from `Pinoc.toml`. Failing that, it prompts with the recommended one (shank, or codama when Codama macros are detected) as the default. When not interactive, it takes the recommended one.
- **Against the recommendation:** passing `--generator` that contradicts the recommendation asks for confirmation; `-y` skips it. Without `-y`, a non-interactive run refuses rather than guess.

## TypeScript

`--language ts` renders a TypeScript client through codama ([`@codama/renderers-js`](https://github.com/codama-idl/renderers-js)) into `clients/ts/`: `src/generated/`, a `src/index.ts` re-export, and a `package.json` with the `@solana/kit` dependencies. It always uses codama, with no prompt; `--generator shank` with it is refused.

```bash
pinoc client generate --language ts --auto-install
```

## Output paths

Each client has its own default directory, so generating several does not overwrite one with another:

| Client | Default | `Pinoc.toml` |
|---|---|---|
| Rust, shank | `clients/rust-shank` | `shank_out_dir` |
| Rust, codama | `clients/rust-codama` | `codama_out_dir` |
| TypeScript | `clients/ts` | `ts_out_dir` |

`--out-dir` wins for one run. A shared `[client] out_dir` applies to whichever Rust generator runs, below the per-generator keys, and warns each run, since the two generators would overwrite each other there.

## Configured clients and `--check`

`[client] languages` lists the clients the project keeps, so a bare `pinoc client generate` renders all of them, reading the IDL from `[idl] out_dir`:

```toml
[client]
languages = ["ts", "rust"]
generator = "codama"         # the Rust client's generator
```

`--language` still renders one client. `--out-dir` names one directory, so it is refused when `languages` asks for more than one and `--language` is not given.

`pinoc client generate --check` renders each client in place, compares, and then puts every file back. The codama tooling in `.pinoc-codama/` stays installed.
- **What it reports:** each file that differs, each that the generator writes but is missing, and each under `src/generated/` it no longer writes. It exits nonzero if there is any.
- **Ignored:** files outside `src/generated/` that the generator does not write, such as a hand-written script beside the client.
- **Run it in the repository:** the TypeScript renderer formats with prettier, which reads the nearest prettier config above the client directory. A copy without the repository's `.prettierrc` renders differently formatted files and reports them all.

Both generators own `src/generated/` and clear it before writing, so an account or instruction removed from the program does not leave its file behind.

## Codama dependencies

Codama's npm dependencies live in a project-local `<out-dir>/.pinoc-codama/`.
- **Consent first:** pinoc installs them only with `--auto-install`. Without it, it stops before writing anything (no directory, no `.gitignore` change) and says so.
- **Gitignored:** `.pinoc-codama/` is added to `.gitignore` when the project is a git repository.
- **No Node.js:** if it is missing, pinoc prints an install pointer.

**The versions are fixed by pinoc, not by npm.** Each pinoc release names exact versions and carries a lock file for their whole dependency tree, and installs with `npm ci`, which installs exactly that tree or fails. The same pinoc therefore renders with the same packages on any machine, and a renderer upgrade is a pinoc change:

| | Rust client | TypeScript client |
|---|---|---|
| `codama` | 1.11.0 | 1.11.0 |
| `@codama/nodes-from-anchor` | 1.5.6 | 1.5.6 |
| renderer | `@codama/renderers-rust` 1.2.10 | `@codama/renderers-js` 2.5.0 |

Every run prints the versions it rendered with, and checks the installed tree first:
- **It came from this pinoc's lock.** pinoc records the lock an install came from (`.pinoc-codama/installed-lock.json`). A tree installed by another pinoc version, or by a hand-run `npm install`, is not used, so upgrading pinoc across a lock change needs `--auto-install` once.
- **It has not changed since.** `npm ci` verifies integrity hashes only while installing, so pinoc also records a fingerprint of every installed file (`.pinoc-codama/installed-tree`) and compares it on each run. An edited, added or removed file, or a cache that restored a different tree, is refused. This notices a change; it is not a defence against someone who rewrites the fingerprint too.

Either way the command stops before rendering and asks for `--auto-install`, which reinstalls. A hand-run `npm ci` inside `.pinoc-codama/` restores the same tree and is accepted. Each output directory has its own `.pinoc-codama/`, and the two languages use different renderer packages, so a Rust and a TypeScript client install two `node_modules` trees, about 35 MB each.

## CPI variants

When the program source has `invoke`/`invoke_signed` call sites, the shank generator emits `XxxCpi`, `XxxCpiAccounts` and `XxxCpiBuilder` for each instruction, for calling this program from another one, plus the dependencies they need (`solana-account-info`, `solana-cpi`, `solana-program-error`). These helpers take `solana_account_info::AccountInfo`, so the calling program must be solana-program-style, not pinocchio.

```bash
pinoc client generate --with-cpi   # always generate them
pinoc client generate --no-cpi     # never generate them
```

## `fetch_*` RPC helpers

`fetch_x`, `fetch_all_x`, `fetch_maybe_x` and `fetch_all_maybe_x` are generated per account, behind a `fetch` Cargo feature, so `solana-rpc-client` and `solana-account` are pulled in only when asked for: `cargo build --features fetch` in the generated crate.

## Program-derived addresses

When the IDL declares PDAs, the codama Rust client gets `find_program_address` helpers on its accounts. `solana-pubkey` provides the derivation functions only behind its `curve25519` feature, so the generated `Cargo.toml` enables it when the client uses them, and leaves the dependency plain otherwise. Without it the client would fail to build with `E0599: no function or associated item named create_program_address found for struct Pubkey`, which does not mention features.

## Enums with explicit discriminants

An enum whose variants carry values that are not their positions (`Open = 1, Active = 2, Closed = 5`) is generated with those values by every generator.
- **Why this needs doing:** none of the renderers does it on its own. borsh and `@solana/kit` encode an enum by variant position, and `@codama/renderers-js` and `@codama/renderers-rust` drop the discriminators the IDL carries, so the client would write and read the wrong byte with no error (upstream: [renderers-js#6](https://github.com/codama-idl/renderers-js/issues/6)).
- **`shank`** reads the values from the program source, since the shank IDL does not record them, and emits `Open = 1` with `#[borsh(use_discriminant = true)]`.
- **`codama`** rewrites the rendered enum: explicit values, plus `#[borsh(use_discriminant = true)]` in Rust or `{ useValuesAsDiscriminators: true }` on the TypeScript codecs.
  - If the rendered code is not what the rewrite expects, the command fails and says the client must not be used. The renderer versions are fixed by pinoc, so that can only follow a pinoc upgrade.
  - Output that already has the right values is accepted as it is.

`pinoc client generate` prints which enums this applied to. Two shapes cannot be corrected and are refused: explicit non-positional discriminators on an enum whose variants carry data, and on an enum declared inline instead of as its own type. An enum numbered `0..n` is generated as usual.

## Zero-copy layout

The generated client (de)serializes as packed borsh, while scaffolded programs read zero-copy through `#[repr(C)]`. Keep `#[repr(C)]` IDL structs free of padding so the two agree; see the [padding warning](idl.md#zero-copy-padding-warning).
