# Changelog

All notable changes to the `pinoc` CLI tool will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- `pinoc idl [--out-dir]`: generates an Anchor-style IDL JSON (instructions, accounts, types, errors) at `target/idl/<name>.json` by default, using [shank](https://github.com/metaplex-foundation/shank)'s `shank_idl` crate directly, compiled into `pinoc` itself — no separate `shank-cli` install required. `pinoc build` now also regenerates the IDL automatically after a successful build, matching `anchor build`'s behavior; a failed IDL extraction only warns, it never fails the build. The `--with-example` template's instructions/accounts/errors are annotated with `shank`'s derive macros so this works immediately.
- `pinoc client generate [--out-dir] [--idl-dir]`: generates a standalone Rust client crate (`clients/rust/`) from the IDL — Borsh-based instruction builders and account (de)serialization, using lightweight `solana-pubkey`/`solana-instruction` crates. Pure Rust, no Node.js/npm, styled after Codama's Rust renderer conventions (confirmed by running the real Codama pipeline on the Solana Foundation's `pinocchio-counter` template and comparing output). Verified correct end-to-end: an instruction built by the generated client was submitted through `mollusk-svm` against a real compiled program and succeeded.
- `pinoc idl` now also writes `target/idl/<name>.codama.json` alongside the native shank IDL: the same IDL with pubkey-typed fields rewritten from `{"defined": "Address"}` to the standard `"publicKey"` type, working around a bug in `@codama/nodes-from-anchor`'s conversion of shank's native representation (see `docs/codama-comparison.md`). `pinoc client generate` keeps reading the plain (non-rewritten) IDL, since pinoc's own generator already handles shank's representation directly.
- `pinoc client generate --generator <shank|codama>`: choose between pinoc's built-in generator (default, no setup) and shelling out to the real Codama JS pipeline for richer output (CPI helpers, `fetch_*` RPC helpers). Prompts interactively when `--generator` is omitted and stdin is a terminal; defaults to `shank` otherwise. The `codama` path never installs its npm dependencies without explicit consent — it stops with the exact `npm install` command to run unless `--auto-install` is passed. Verified end-to-end: generated crate compiles cleanly with both generators.
- `pinoc idl --program-id <ADDRESS>`: overrides the program address used in the IDL, for programs that declare their ID via `Address::from_str_const` instead of `declare_id!` (shank can't find the address in the source without one or the other). Found while testing pinoc against a real independently-written Pinocchio program (not a pinoc scaffold); `pinoc build`/`pinoc test` already worked unmodified against it, this closes the IDL gap.
- `pinoc build --program-id <ADDRESS>`: same override, threaded into the automatic post-build IDL generation.
- `pinoc idl` now detects error codes from a plain enum with a manual `impl From<X> for ProgramError`, not just enums deriving `thiserror::Error` (all `shank_idl` recognizes on its own). Messages are synthesized from the variant name (`InvalidPda` -> "Invalid Pda") since there's no `#[error("...")]` to read.
- `pinoc client generate --generator codama` now adds `.pinoc-codama/` to the project's `.gitignore` automatically: creates the file if the project is a git repo, appends to it if one already exists, and leaves non-git projects untouched.
- `pinoc idl`/`pinoc build` now auto-detect Codama's own Rust derive macros (`CodamaAccount`, `CodamaInstructions`, `CodamaErrors`, `CodamaEvent(s)`, `CodamaPda`, `CodamaType`; requires a `codama` dependency plus at least one of these derives) and, when found, generate `.codama.json` via Codama's real Rust extractor (`codama::Codama::load(...).get_json_idl()`) instead of the shank-IDL-plus-shim path — no `@codama/nodes-from-anchor` conversion involved. Since Codama's own `declare_id!` detection doesn't recognize Pinocchio's macro path, the address pinoc already resolved via shank is injected into the native output so both `.json` and `.codama.json` agree. The plain `<name>.json` is unaffected either way — it's always shank's extraction. Native output is identifiable by a top-level `"kind": "rootNode"` field; shim output isn't. Verified end-to-end against a real Codama-annotated program (Solana Foundation's `pinocchio-counter` template): correct instruction/account extraction, correct address injection, and a complete, compiling generated client.
- `--idl-generator <shank|codama>` on both `pinoc idl` and `pinoc build`: forces the `.codama.json` generator instead of auto-detecting, for either direction (force native extraction on an unannotated program, or force the shim on an annotated one).
- `[idl].generator = "auto" | "shank" | "codama"` in `Pinoc.toml` (new section, also added to the generated template): a persistent per-project default for the same choice. Precedence is `--idl-generator` > `Pinoc.toml` > auto-detect.
- `pinoc client generate` now uses the same Codama-macro detection to recommend a generator: the interactive prompt's default (on bare Enter) flips to whichever was detected. If `--generator` is passed explicitly and contradicts detection, a confirmation prompt appears; `-y`/`--yes` skips it. Non-interactively (no TTY) without `-y` in that situation, it refuses with a message telling you to add the flag, rather than guessing or hanging.

### Changed
- **Breaking**: `pinoc init` now generates the minimal no-op program by default (previously behind `--no-boilerplate`). The full PDA-account example is now opt-in via `--with-example` (replaces `--no-boilerplate`, which is removed).
- Bumped generated project dependencies to latest stable: `pinocchio` 0.8.4 → 0.11.2, `pinocchio-log` 0.4.0 → 0.5.1, `pinocchio-system` 0.2.3 → 0.6.1, `shank` 0.4.2 → 0.4.8, `solana-sdk` 2.3.0 → 4.0.1, `solana-program-runtime` 2.3.1 → 4.1.2, `mollusk-svm`/`mollusk-svm-bencher` 0.3.0 → 0.14.0. Added `thiserror` (needed for `shank`'s error-IDL convention).
- Dropped `pinocchio-pubkey` from generated projects: it hasn't been updated since pinocchio 0.9 and no longer matches pinocchio's `Address` API. Program IDs are now declared with `pinocchio::address::declare_id!` (`solana-address`'s own macro, re-exported through pinocchio), matching the Solana Foundation's own Pinocchio template.
- Updated all scaffold templates for pinocchio 0.11's API: `AccountInfo` → `AccountView`, `Pubkey` → `Address`, `.key()` → `.address()`, `.data_is_empty()` → `.is_data_empty()`, `pinocchio::program_error` → `pinocchio::error`, `msg!` → `pinocchio_log::log!`, `Rent::from_account_info` → `Rent::from_account_view`, `Rent::minimum_balance` → `Rent::try_minimum_balance`.
- `pinoc build`'s IDL-generation warning now prints the full underlying error instead of a generic one-liner, plus a hint to pass `--program-id` if the program doesn't use `declare_id!`.
- The printed `npm install` instructions for the `codama` generator's first run are now a true one-liner (`npm install --prefix <dir>`) instead of `cd <dir> && npm install`.
- Reorganized `src/`: `main.rs` now only holds CLI parsing and dispatch; each command lives in `src/commands/`, IDL generation/transformation in `src/idl/`, and client generation in `src/client_gen/{shank,codama}/`.

### Fixed
- `#[idl_type("publicKey")]` field overrides produced `{"defined": "publicKey"}` in the IDL rather than the literal `"publicKey"` type. This broke `pinoc client generate --generator shank` (emitted a reference to an undefined `PublicKey` type) and left those fields unrewritten in `.codama.json`. Both now treat `Defined("publicKey")` the same as `Defined("Address")`.

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
