# Changelog

All notable changes to the `pinoc` CLI tool will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `pinoc check`: a lint command for Solana-specific safety issues that rustc and clippy do not model (account validation, CPI, zero-copy layout). This lands the scaffolding only — the finding/lint contract, `Pinoc.toml` `[check]` config (`deny`/`warn`/`allow`/`confidence_threshold`), `--deny`/`--allow` flags (accepting `*` for all codes), inline `// pinoc:allow(CODE)` suppression, human and `--json` output, and the exit code (nonzero only when a surviving finding is `deny`). No checks are implemented yet; the command reports no issues until lints are added.
- GitHub Actions CI (`.github/workflows/ci.yml`), run on pushes to `main` and on every pull request: `cargo build --locked` and `cargo fmt --all -- --check` as required checks, plus a non-blocking `cargo clippy --all-targets` job for informational lint results. A CI status badge is added to the README.

### Changed
- Repo-wide `cargo fmt` pass so the formatting check starts from a clean baseline.

## [0.2.0] - 2026-07-16

### Added
- `pinoc idl [--out-dir]`: generates an Anchor-style IDL JSON (instructions, accounts, types, errors) at `target/idl/<name>.json` by default, using [shank](https://github.com/metaplex-foundation/shank)'s `shank_idl` crate directly, compiled into `pinoc` itself — no separate `shank-cli` install required. `pinoc build` now also regenerates the IDL automatically after a successful build, matching `anchor build`'s behavior; a failed IDL extraction only warns, it never fails the build. The `--with-example` template's instructions/accounts/errors are annotated with `shank`'s derive macros so this works immediately.
- `pinoc client generate [--out-dir] [--idl-dir]`: generates a standalone Rust client crate (`clients/rust/`) from the IDL — Borsh-based instruction builders and account (de)serialization, using lightweight `solana-pubkey`/`solana-instruction` crates. Pure Rust, no Node.js/npm, styled after Codama's Rust renderer conventions (confirmed by running the real Codama pipeline on the Solana Foundation's `pinocchio-counter` template and comparing output). Verified correct end-to-end: an instruction built by the generated client was submitted through `mollusk-svm` against a real compiled program and succeeded.
- `pinoc idl` now also writes `target/idl/<name>.codama.json` alongside the native shank IDL: the same IDL with pubkey-typed fields rewritten from `{"defined": "Address"}` to the standard `"publicKey"` type, working around a bug in `@codama/nodes-from-anchor`'s conversion of shank's native representation. `pinoc client generate` keeps reading the plain (non-rewritten) IDL, since pinoc's own generator already handles shank's representation directly.
- `pinoc client generate --generator <shank|codama>`: choose between pinoc's built-in generator (no setup) and shelling out to the real Codama JS pipeline for richer output. Prompts interactively when `--generator` is omitted and stdin is a terminal. The `codama` path never installs its npm dependencies without explicit consent — it stops with the exact `npm install` command to run unless `--auto-install` is passed. Verified end-to-end: generated crate compiles cleanly with both generators. (Since superseded by the recommend/confirm flow described below, and `shank` now also has CPI/fetch helpers, closing the "richer output" gap.)
- `pinoc idl --program-id <ADDRESS>`: overrides the program address used in the IDL, for programs that declare their ID via `Address::from_str_const` instead of `declare_id!` (shank can't find the address in the source without one or the other). Found while testing pinoc against a real independently-written Pinocchio program (not a pinoc scaffold); `pinoc build`/`pinoc test` already worked unmodified against it, this closes the IDL gap.
- `pinoc build --program-id <ADDRESS>`: same override, threaded into the automatic post-build IDL generation.
- `pinoc idl` now detects error codes from a plain enum with a manual `impl From<X> for ProgramError`, not just enums deriving `thiserror::Error` (all `shank_idl` recognizes on its own). Messages are synthesized from the variant name (`InvalidPda` -> "Invalid Pda") since there's no `#[error("...")]` to read.
- `pinoc client generate --generator codama` now adds `.pinoc-codama/` to the project's `.gitignore` automatically: creates the file if the project is a git repo, appends to it if one already exists, and leaves non-git projects untouched.
- `pinoc idl`/`pinoc build` now auto-detect Codama's own Rust derive macros (`CodamaAccount`, `CodamaInstructions`, `CodamaErrors`, `CodamaEvent(s)`, `CodamaPda`, `CodamaType`; requires a `codama` dependency plus at least one of these derives) and, when found, generate `.codama.json` via Codama's real Rust extractor (`codama::Codama::load(...).get_json_idl()`) instead of the shank-IDL-plus-shim path — no `@codama/nodes-from-anchor` conversion involved. Since Codama's own `declare_id!` detection doesn't recognize Pinocchio's macro path, the address pinoc already resolved via shank is injected into the native output so both `.json` and `.codama.json` agree. The plain `<name>.json` is unaffected either way — it's always shank's extraction. Native output is identifiable by a top-level `"kind": "rootNode"` field; shim output isn't. Verified end-to-end against a real Codama-annotated program (Solana Foundation's `pinocchio-counter` template): correct instruction/account extraction, correct address injection, and a complete, compiling generated client.
- `--idl-generator <shank|codama>` on both `pinoc idl` and `pinoc build`: forces the `.codama.json` generator instead of auto-detecting, for either direction (force native extraction on an unannotated program, or force the shim on an annotated one).
- `[idl].generator = "auto" | "shank" | "codama"` in `Pinoc.toml` (new section, also added to the generated template): a persistent per-project default for the same choice. Precedence is `--idl-generator` > `Pinoc.toml` > auto-detect.
- `pinoc client generate` now uses the same Codama-macro detection to recommend a generator: `codama` if detected, `shank` otherwise. The interactive prompt prints which one and marks it `(recommended)`; bare Enter picks it. If `--generator` is passed explicitly and contradicts the recommendation, a confirmation prompt appears; `-y`/`--yes` skips it. Non-interactively (no TTY) without `-y` in that situation, it refuses with a message telling you to add the flag, rather than guessing or hanging.
- `--out-dir`'s default for `pinoc client generate` now depends on the resolved generator (`clients/rust-shank` or `clients/rust-codama`) instead of a single fixed `clients/rust` both generators shared. Previously, generating with one generator and then the other silently overwrote the first's output; now both coexist by default, and `--out-dir` still overrides either.
- `[client]` section in `Pinoc.toml` (not added by default in scaffolded projects): `out_dir` sets a shared path both generators use, printing a warning every time it's used since switching generators would overwrite the other's output there; `shank_out_dir`/`codama_out_dir` set per-generator paths and win over the shared `out_dir`. Precedence is `--out-dir` (CLI) > per-generator config > shared config > the dynamic default. If `Pinoc.toml` has no `[client]` section at all, behavior is unchanged from the dynamic default above.
- `pinoc deploy` no longer requires `Pinoc.toml` to exist. If it's missing, cluster/wallet defaults now come from `solana config get` (the Solana CLI's own config) instead of failing outright; `--cluster`/`--wallet` still override individual fields either way, and are skipped entirely (no `Pinoc.toml`/`solana config get` lookup at all) when both are passed explicitly.
- `pinoc config init [-y]`: creates a `Pinoc.toml` for the current project on demand (a no-op if one already exists). Refuses on a project with no `pinocchio` dependency in `Cargo.toml` unless `-y`/`--yes` is passed, or bails with that same hint non-interactively. Any command that reads `Pinoc.toml` and finds it missing now prints a one-line pointer to this command instead of just silently falling back.
- `pinoc client generate --generator shank` now produces CPI variants (`XxxCpi`/`XxxCpiAccounts`/`XxxCpiBuilder`, for calling this program's instructions from inside another program via cross-program invocation) and `fetch_*` RPC helpers (`fetch_x`/`fetch_all_x`/`fetch_maybe_x`/`fetch_all_maybe_x` per account), closing two gaps against the real Codama generator. Verified for exact API-shape parity against real Codama output (an actual `renderVisitor` run, cross-checked against the renderer's own `.njk` template source). CPI variants are auto-detected: generated only when the program's own source actually calls `invoke`/`invoke_signed`, overridable with `--with-cpi`/`--no-cpi` (adds 3 dependencies only when generated, unlike real Codama which always includes them). `fetch_*` helpers are always generated but gated behind a real `fetch` Cargo feature with `solana-rpc-client`/`solana-account` marked `optional = true`, unlike real Codama's own output, which emits `#[cfg(feature = "fetch")]` code without ever declaring the feature or its dependencies unless the `syncCargoToml` renderer option is used.
- Zero-copy layout safety for `#[repr(C)]` IDL structs. The generated client (de)serializes args and accounts as packed borsh, but the scaffold reads them zero-copy (`load_ix_data`/`load_acc_mut_unchecked`), so implicit alignment padding makes the two disagree. Scaffolded `#[repr(C)]` IDL structs now carry a compile-time `assert!(size_of::<T>() == ...)`, and `pinoc build`/`pinoc idl` warn when a `ShankAccount`/`ShankType` `#[repr(C)]` struct has implicit padding, pointing at the `_padding: [u8; N]` fix.
- `src/idl/README.md` and `src/client_gen/README.md`: reference docs for the IDL and client generators (output files, generator selection, CPI/`fetch_*` variants, output-path precedence, the zero-copy padding lint). The top-level `README.md` links to them instead of carrying the full detail inline.
- `pinoc client generate --generator shank` now renders `enum` defined types (unit, tuple, and named/struct variants), for both standalone types and enum-typed instruction args and struct fields; it previously aborted with "does not support enum types yet". Borsh output is byte-identical to the codama generator (`u8` variant index followed by the variant's fields).

### Changed
- **Breaking**: `pinoc init` now generates the minimal no-op program by default (previously behind `--no-boilerplate`). The full PDA-account example is now opt-in via `--with-example` (replaces `--no-boilerplate`, which is removed).
- Bumped generated project dependencies to latest stable: `pinocchio` 0.8.4 → 0.11.2, `pinocchio-log` 0.4.0 → 0.5.1, `pinocchio-system` 0.2.3 → 0.6.1, `shank` 0.4.2 → 0.4.8, `solana-sdk` 2.3.0 → 4.0.1, `solana-program-runtime` 2.3.1 → 4.1.2, `mollusk-svm`/`mollusk-svm-bencher` 0.3.0 → 0.14.0. Added `thiserror` (needed for `shank`'s error-IDL convention).
- Dropped `pinocchio-pubkey` from generated projects: it hasn't been updated since pinocchio 0.9 and no longer matches pinocchio's `Address` API. Program IDs are now declared with `pinocchio::address::declare_id!` (`solana-address`'s own macro, re-exported through pinocchio), matching the Solana Foundation's own Pinocchio template.
- Updated all scaffold templates for pinocchio 0.11's API: `AccountInfo` → `AccountView`, `Pubkey` → `Address`, `.key()` → `.address()`, `.data_is_empty()` → `.is_data_empty()`, `pinocchio::program_error` → `pinocchio::error`, `msg!` → `pinocchio_log::log!`, `Rent::from_account_info` → `Rent::from_account_view`, `Rent::minimum_balance` → `Rent::try_minimum_balance`.
- `pinoc build`'s IDL-generation warning now prints the full underlying error instead of a generic one-liner, plus a hint to pass `--program-id` if the program doesn't use `declare_id!`.
- The printed `npm install` instructions for the `codama` generator's first run are now a true one-liner (`npm install --prefix <dir>`) instead of `cd <dir> && npm install`.
- Reorganized `src/`: `main.rs` now only holds CLI parsing and dispatch; each command lives in `src/commands/`, IDL generation/transformation in `src/idl/`, and client generation in `src/client_gen/{shank,codama}/`.
- `PinocConfig`/`ProviderConfig` moved out of `src/commands/deploy.rs` into a new shared `src/config.rs`, since `pinoc idl`/`pinoc build` now also read `Pinoc.toml` (for `[idl].generator`) and it's no longer deploy-specific. `read_pinoc_config_optional` returns `None` instead of erroring when the file is simply absent, so both IDL generation and `deploy` can fall back to their own defaults outside a `Pinoc.toml` project.

### Fixed
- `#[idl_type("publicKey")]` field overrides produced `{"defined": "publicKey"}` in the IDL rather than the literal `"publicKey"` type. This broke `pinoc client generate --generator shank` (emitted a reference to an undefined `PublicKey` type) and left those fields unrewritten in `.codama.json`. Both now treat `Defined("publicKey")` the same as `Defined("Address")`.
- `--idl-generator shank` on a program that actually has Codama macros used to print "no Codama macros detected" (misleading — they were detected, just overridden). Now correctly says the choice was forced.
- `[idl].generator` in `Pinoc.toml` was case-sensitive (`"Auto"`/`"Shank"` errored instead of working) while the equivalent `--idl-generator` CLI flag wasn't. Now matched case-insensitively like the flag.
- Forcing `--idl-generator codama`/`[idl].generator = "codama"` on a program with no Codama macros silently wrote a completely empty `.codama.json` with exit code 0. Now prints a warning when the extracted program has zero instructions, accounts, and errors.
- `pinoc build`'s "Skipped IDL generation" hint always suggested `--program-id`, even for unrelated failures (e.g. an invalid `[idl].generator` value in `Pinoc.toml`). Now only shown when actually relevant to the error, plus a matching hint for bad `[idl].generator` values.
- `pinoc client generate --generator codama` failed to compile for any program with zero accounts (not just fully-empty ones — an instructions-only program would hit this too): the generated `lib.rs` unconditionally re-exported `generated::accounts::*`, but the real Codama renderer only emits `accounts/`, `instructions/`, or `types/` when that category actually has content. `lib.rs` generation is now built from what actually got rendered instead of a fixed template. Pre-existing bug, not something this session introduced; every prior test fixture happened to have at least one account so it never surfaced.
- `pinoc deploy` deployed to a fresh random program address every run (it never passed `--program-id`), so the on-chain id never matched `declare_id!` and each deploy created a new program instead of upgrading. It now passes `target/deploy/<name>-keypair.json` as `--program-id` when present, deploying to the declared id and upgrading in place.
- `pinoc keys sync` rewrote the id declaration to a hardcoded `pinocchio_pubkey::declare_id!(...)`, breaking current-template projects (which use `pinocchio::address::declare_id!` and don't depend on `pinocchio-pubkey`). It now locates the declaration across the whole `src/` tree via `syn` (any `declare_id!` macro path, or a `const ID` initialized from `Address::from_str_const`/`pubkey!`) and rewrites only that address literal, preserving the declaration form. This also lets it sync programs that declare their id outside `lib.rs` (e.g. in `constants.rs`), which it previously could not do. `keys sync` also failed with "Could not find project name" on a `Cargo.toml` whose `name` field had padded alignment (`name    = "..."`); it now parses the manifest with `toml`.
- `pinoc client generate --generator shank` emitted an empty `XxxCpiAccounts<'a, 'b>` for a zero-account instruction when CPI variants were on, failing to compile (`E0392`, unused lifetime). It now skips the accounts struct and its `new()` parameter for zero-account instructions, matching Codama.
- `pinoc client generate --generator shank` emitted invalid Rust when an account or argument was named a Rust keyword (e.g. an account named `match` produced `pub fn ...(match: Pubkey, ...)`). Field, parameter, function, and module names that collide with keywords are now emitted as raw identifiers (`r#match`).
- `pinoc client generate --generator shank` aborted with "does not support the IDL type Bytes" for any `Vec<u8>` field (shank represents `Vec<u8>` as the `bytes` IDL type). `Bytes` now maps to `Vec<u8>`.
- Generated account/type structs derived `Copy` unconditionally, so any struct with a `Vec`/`String` field failed to compile (`E0204`). The `Copy` derive is dropped (`Clone` is kept).
- A generated account/type struct with a field of another defined (nested) type did not import it, failing to compile (`E0412`). The `use crate::{...}` import is now generated for account and type struct fields as well (recursing through `Array`/`Vec`/`Option`), not just for instruction arguments.

## [0.1.7] - 2026-07-13

### Added
- `--quiet`/`-q` flag for `pinoc build` and `pinoc test` to suppress verbose cargo output. On `test`, a failing run still prints the panic message/assertion diff instead of hiding it.

### Changed
- Split `src/content.rs` into a `src/templates/` module with per-topic submodules (`instructions`, `minimal`, `states`, `unit_tests`) for maintainability.
- Corrected `Cargo.toml`'s `license` field from `MIT` to `Apache-2.0`, matching the actual `LICENSE` file and README.

### Fixed
- `pinoc keys list` header row column widths now match the separator row below it.

## [0.1.6] - 2026-07-13

### Fixed
- `pinoc clean --no-preserve` could transiently fail with a "Directory not empty" error immediately after a build, due to a race with file handles not yet released by a prior cargo process. `remove_dir_all` is now retried a few times with a short backoff before giving up.

## [0.1.5] - 2025-07-17

### Added
- `Pinoc.toml` configuration file for deployment settings
  - Automatic generation of `Pinoc.toml` during project initialization
  - Support for cluster and wallet configuration
  - `pinoc deploy` now reads from `Pinoc.toml` instead of requiring manual parameters
  - Home directory expansion support for wallet paths (e.g., `~/.config/solana/id.json`)
- Command-line override options for deployment configuration
  - `--cluster` flag to override cluster URL from Pinoc.toml
  - `--wallet` flag to override wallet path from Pinoc.toml
  - Both flags are optional and fall back to Pinoc.toml values when not provided
- Enhanced deployment feedback showing cluster and wallet being used
- TOML configuration parsing with proper error handling

### Changed
- `pinoc deploy` command now uses configuration from `Pinoc.toml` file with optional command-line overrides
- Improved deployment process with better user feedback and error messages

## [0.1.4] - 2025-07-17

### Added
- `--no-git` flag for `pinoc init` to skip git repository initialization. When used, the generated project will not be initialized with git, and `cargo init` will use `--vcs none` for a clean setup without version control.
- `--no-boilerplate` flag for `pinoc init` to create minimal projects without tests and boilerplate code

### Changed
- Updated dev-dependencies in generated projects to use latest compatible versions:
  - `solana-sdk` updated to "2.3.0"
  - `solana-program-runtime` updated to "=2.3.1"
  - `mollusk-svm` updated to "0.3.0"
  - `mollusk-svm-bencher` updated to "0.3.0"

## [0.1.3] - 2025-07-17

### Added

- `pinoc keys` subcommand for program key management
  - `pinoc keys list` - List all program keypairs with their public keys and file locations
  - `pinoc keys sync` - Sync program ID in lib.rs with keypair, with consistency checking
- Smart consistency checking in `pinoc keys sync` to prevent unnecessary file updates
- Enhanced user feedback for key management operations

## [0.1.2] - 2025-07-17

### Added

- `pinoc clean` command to remove target directory while preserving keypair files
- `--no-preserve` flag for `pinoc clean` to skip preserving keypair files
- Binary configuration in Cargo.toml for proper installation via `cargo install pinoc`
- Comprehensive help documentation for all commands

### Changed

- Updated project structure to support crates.io publication
- Enhanced error handling and user feedback messages
- Improved command-line interface with better help text

### Fixed

- Fixed authors field syntax in Cargo.toml
- Resolved compilation issues and improved code structure

## [0.1.1] - 2024-07-17

### Added

- `pinoc clean` command with keypair preservation functionality
- `--no-preserve` flag for complete target directory cleanup
- Enhanced help banner with new command documentation
- Improved error handling and user feedback

### Changed

- Updated Cargo.toml to include binary configuration for `cargo install`
- Enhanced command structure to support flags and options
- Improved code organization and maintainability

## [0.1.0] - 2024-07-17

### Added

- Initial release of `pinoc` CLI tool
- `pinoc init <project_name>` - Initialize new Pinocchio projects
- `pinoc build` - Build Solana programs
- `pinoc test` - Run project tests
- `pinoc deploy` - Deploy programs to Solana
- `pinoc add <package_name>` - Add packages to projects
- `pinoc search [query]` - Search for Pinocchio packages
- Project scaffolding with proper directory structure
- Automatic keypair generation during project initialization
- Git repository initialization with initial commit
- Comprehensive project templates and boilerplate code

### Features

- Fast project scaffolding with best practices
- Proper directory structure for Solana/Pinocchio development
- Simple build, test, and deployment commands
- Comprehensive testing environment setup
- Automatic dependency management
- Package search functionality

---

## Installation

### From crates.io (Recommended)

```bash
cargo install pinoc
```

### From GitHub

```bash
cargo install --git https://github.com/a91y/pinoc --force
```

### From Source

```bash
git clone https://github.com/a91y/pinoc.git
cd pinoc
cargo install --path .
```

## Usage

```bash
# Initialize a new project
pinoc init my-project

# Build your project
pinoc build

# Run tests
pinoc test

# Deploy your program
pinoc deploy

# Clean target directory (preserves keypairs)
pinoc clean

# Clean target directory (removes everything including keypairs)
pinoc clean --no-preserve

# Add a package
pinoc add package-name

# Search for packages
pinoc search query

# Manage program keys
pinoc keys list          # List all program keypairs
pinoc keys sync          # Sync program ID in lib.rs with keypair

# Get help
pinoc --help
```
