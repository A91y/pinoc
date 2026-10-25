<div align="center">
  <img src="assets/logo.png" alt="Pinoc CLI Logo" width="20%">
  <h1>Pinoc</h1>
  <p><strong>Scaffold, build, and ship Solana Pinocchio programs, fast.</strong></p>

[![CI](https://github.com/A91y/pinoc/actions/workflows/ci.yml/badge.svg)](https://github.com/A91y/pinoc/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/pinoc)](https://crates.io/crates/pinoc)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-yellow.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/rust-1.70+-blue.svg)](https://www.rust-lang.org)
[![Downloads](https://img.shields.io/crates/d/pinoc)](https://crates.io/crates/pinoc)

  <a class="header-badge" target="_blank" href="https://twitter.com/AyushAgr91">
    <img alt="Twitter" src="https://img.shields.io/badge/@AyushAgr91-000000?style=for-the-badge&logo=x&logoColor=white">
  </a>
</div>

---

A Rust CLI for [Pinocchio](https://github.com/anza-xyz/pinocchio) programs. It scaffolds a project, builds and deploys it, keeps program IDs in sync, generates an IDL and a standalone Rust client from it, and lints for Solana-specific safety issues. Sensible defaults, no required configuration.

## Installation

```bash
cargo install pinoc
# or, a prebuilt binary
cargo binstall pinoc
```

<details>
<summary>Other methods</summary>

```bash
# Latest from GitHub
cargo install --git https://github.com/A91y/pinoc --force

# From source
git clone https://github.com/A91y/pinoc.git
cd pinoc && cargo install --path .
```

</details>

## Quick start

```bash
pinoc init my_app       # scaffold a project
cd my_app
pinoc build             # build + regenerate the IDL
pinoc test              # run tests (mollusk-svm)
pinoc check             # lint for Solana safety issues
pinoc deploy            # deploy to the configured cluster
```

## Commands

| Command | Description |
| --- | --- |
| `pinoc init <name>` | Create a new project |
| `pinoc build` | Build the program and regenerate the IDL |
| `pinoc test` | Build the program, then run tests |
| `pinoc check` | Lint for Solana-specific safety issues (account, CPI, zero-copy) |
| `pinoc deploy` | Deploy to a cluster |
| `pinoc verify` | Compare the deployed program with the local artifact |
| `pinoc clean` | Clean build artifacts (keypairs preserved) |
| `pinoc add <package>` | Add a Pinocchio package |
| `pinoc search [query]` | Search packages |
| `pinoc keys list` | List program keypairs |
| `pinoc keys sync` | Sync the program ID in source with its keypair |
| `pinoc idl` | Regenerate the IDL JSON |
| `pinoc client generate` | Generate a Rust or TypeScript client from the IDL |
| `pinoc config init` | Create a `Pinoc.toml` for the project |

Common options:

- `pinoc init <name> --with-example`: scaffold a worked PDA-account example instead of a no-op program
- `pinoc init <name> --no-git`: skip git initialization
- `pinoc deploy --cluster <cluster> --wallet <path>`: override deployment settings (`--cluster` takes a `[clusters]` name or a URL/moniker)
- `pinoc deploy --program-keypair <path> --upgrade-authority <path>`: the program's keypair and its upgrade authority (see [Deploying](#deploying))
- `pinoc build --cluster <name>` / `pinoc test --cluster <name>`: build for a `[clusters]` entry, with its features (see [Clusters](#clusters))
- `pinoc test --all-clusters`: run the tests once per `[clusters]` feature set
- `pinoc build --program-id <ADDRESS>`: set the IDL program address for programs that don't call `declare_id!`
- `pinoc build --features <FEATURES>` / `pinoc test --features <FEATURES>`: activate cargo features, passed to `cargo build-sbf` and `cargo test` (repeatable or comma-separated, like cargo's own flag)
- `pinoc build --arch <v0|v1|v2|v3|v4>` / `pinoc test --arch <..>`: the SBPF version to build for, passed to `cargo build-sbf` (see [SBPF version](#sbpf-version))
- `pinoc test --build-features <FEATURES>`: features for the pre-test SBF build when they differ from the test features (`""` for none)
- `pinoc test --no-build`: skip the SBF build and test against the existing `target/deploy/*.so`
- `pinoc test -- <CARGO_ARGS>`: everything after `--` goes to `cargo test` (`pinoc test -- --test client`, `pinoc test -- my_test -- --nocapture`)
- `pinoc client generate --language ts`: generate a TypeScript client (codama generator) into `clients/ts`
- `pinoc idl --check` / `pinoc client generate --check`: write nothing, exit nonzero if the committed IDL or client differs from the source (see [IDL and client generation](#idl-and-client-generation))
- `pinoc clean --no-preserve`: clean everything, including keypairs

## Testing

`pinoc test` runs `cargo build-sbf` before `cargo test`, because SVM tests (mollusk-svm, litesvm, solana-program-test) load `target/deploy/*.so`, which `cargo test` does not build. Without the build step, tests silently run against the last-built binary.

`--features` applies to both steps, except for any feature that enables `no-entrypoint` (such as the scaffold's `test-default`), which is left out of the build: `no-entrypoint` compiles the program out of the SBF artifact. When the build and the tests need different features, set the build's separately:

```bash
pinoc test --features test-default               # build: no features, tests: test-default
pinoc test --features test-default,devnet --build-features devnet
pinoc test --features test-default --build-features ""   # build with no features at all
```

`--build-features` is used as given. Pass `--no-build` to skip the build when iterating on tests that don't load the `.so`.

Arguments after `--` are handed to `cargo test` unchanged, after the features, so one target or one test can be run without leaving `pinoc test`:

```bash
pinoc test --features test-default -- --test client_roundtrip
pinoc test --features test-default -- parses_the_fixture -- --exact --nocapture
```

### Test configuration

`[test]` in `Pinoc.toml` sets what every run needs, so `pinoc test` alone runs the whole suite:

```toml
[test]
features = ["test-default"]        # added to every `cargo test`, before --features
programs = ["tests/mock_venue"]    # other SBF programs the tests load
```

Each entry in `programs` is a directory with its own `Cargo.toml` (a stand-in for a program the one under test calls). `pinoc test` builds each with `cargo build-sbf` in that directory, with the same `--arch` as the program, after the program's own build and before the tests. `--no-build` skips them too.

### What `--no-build` tests

`pinoc build` and `pinoc test` write `target/deploy/<name>.build.json` beside the artifact they build: its features, its `--arch`, the `[clusters]` entry it was built for (`cluster`, when built with `--cluster`), and its length and hashes (`sha256` is the artifact's SHA-256). The format is stable, so a test harness can read it to tell which build it is loading. `pinoc test --no-build` reads it and stops if the artifact was built with other features or another arch than this run would have used, because tests run against it fail in ways that read as bugs in the program:

```text
Error: target/deploy/prog.so was built with --features devnet, but this run would build it with no features. Drop --no-build to rebuild it, or pass --build-features "devnet" if that is the artifact to test.
```

With `--no-build`, `--build-features` and `--arch` say which artifact is expected. If there is no record, or the artifact is not the one the record describes (built by `cargo build-sbf` directly, say), pinoc says it cannot tell what the artifact is and runs the tests.

### Entrypoint check

`pinoc build`, `pinoc test`, and `pinoc deploy` refuse an SBF artifact with no entrypoint (an ELF entry address outside its executable code, the same rule the SBF loader enforces), which is what `no-entrypoint` produces: it builds fine but contains no program, so no test can load it and a deploy would pay rent for an unusable program. The error names the cause (a build feature, the `default` feature, or a program with no entrypoint). `pinoc build` and `pinoc test` delete the bad artifact; `pinoc deploy` checks whatever it is about to upload, however it was built. Unlike `pinoc test`, `pinoc build` never drops a feature on its own: its features describe the artifact you asked for.

### SBPF version

`cargo build-sbf` builds for the SBPF version given by `--arch`, and for its own default when the flag is absent (`v0` in cargo-build-sbf 4.3). `pinoc build` and `pinoc test` pass `--arch` only when one is set, with the flag or in `Pinoc.toml`:

```toml
[build]
arch = "v3"
```

The flag overrides the file. Setting it in the file keeps `pinoc build` and `pinoc test` on the same version; with the flag alone, a `pinoc test` run without it rebuilds the artifact for the toolchain's default.

Which versions a cluster deploys is decided by two feature gates: one enables SBPFv3, and SIMD-0500 disables deployment of v0, v1 and v2. `solana-test-validator` 4.3 starts with both active, so it deploys only v3. Before uploading, `pinoc deploy` reads the artifact's version from its ELF header and asks the cluster for both gates (`solana feature status`), and stops if the cluster would reject the artifact, naming the `--arch` to rebuild with. If the cluster cannot be asked, the deploy goes ahead unchecked.

## Clusters

A program whose build differs per cluster (venue program ids behind a `devnet` feature, say) names its clusters in `Pinoc.toml`:

```toml
[clusters.mainnet]
url = "mainnet-beta"

[clusters.devnet]
features = ["devnet"]              # url defaults to the name, so this reaches devnet

[clusters.localnet]
url = "localhost"
features = ["devnet"]
wallet = "~/.config/solana/local.json"   # optional, overrides [provider] wallet
# upgrade_authority = "..."             # optional, overrides [provider] upgrade_authority
```

- `pinoc build --cluster devnet` builds with the entry's features (plus any `--features`) and records the cluster in the build record.
- `pinoc test --cluster devnet` builds and tests with them; `[test] features` and `--features` are added.
- `pinoc test --all-clusters` runs the suite once per distinct feature set (here twice: `devnet` and `localnet` share one run), reports every run, and fails if any failed.
- `pinoc deploy --cluster devnet` deploys to the entry's `url`, and refuses an artifact whose build record shows it was built for another cluster: one missing a feature this cluster builds with, or carrying a feature only another cluster builds with. `--cluster` (or `[provider] cluster`) finds an entry by name, or by its `url`, so with `[provider] cluster = "localhost"` a bare `pinoc deploy` is checked as `localnet`. A value that names no entry is passed to `solana --url` unchecked, as before.

```text
Error: target/deploy/prog.so was built with --features devnet (for cluster `devnet`), which is not a build for cluster `mainnet`: `devnet` is a feature of cluster `devnet`. Rebuild it with `pinoc build --cluster mainnet`.
```

An artifact with no build record (built outside pinoc) is reported and deployed unchecked.

## Deploying

`pinoc deploy` deploys `target/deploy/<name>.so` (the package's own artifact, not whichever `.so` comes first) with the program's keypair as `--program-id`, so it lands at the address the program declares and upgrades in place. The keypair is `--program-keypair`, else `[provider] program_keypair`, else `target/deploy/<name>-keypair.json`.

It refuses a keypair that holds another address than the program's `declare_id!` (or `const ID`). This matters on a fresh checkout: `cargo build-sbf` writes a new random keypair at the default path when there is none, and deploying with it would put the program at an address nothing refers to, and report success. Point `program_keypair` at the real keypair (it is usually kept out of the repository):

```toml
[provider]
cluster = "localhost"
wallet = "~/.config/solana/id.json"
program_keypair = "../keys/program.json"
upgrade_authority = "~/.config/solana/admin.json"   # optional, passed as --upgrade-authority
```

With no keypair at all, `pinoc deploy` passes the declared address, which upgrades a program that already exists (a first deploy needs the keypair, and `solana` says so). With no keypair and no declared address, it refuses rather than deploy to a random one.

`pinoc verify [--cluster <name>]` downloads the deployed program (`solana program dump`) and compares it with the local artifact, allowing for the zero-filled space a program account has past the program. It prints both SHA-256 hashes and, when the build record matches, which features and cluster the deployed build was made with. It exits nonzero when they differ, so it can follow every deploy or upgrade.

## Project structure

`pinoc init` produces a blank, buildable program with a single no-op instruction:

```
my_app/
├── Cargo.toml
├── Pinoc.toml           # deployment configuration
├── src/lib.rs
└── target/deploy/my_app-keypair.json
```

<details>
<summary><code>--with-example</code> layout</summary>

A full PDA-account creation example, annotated so `pinoc idl` works out of the box:

```
my_app/
├── src/
│   ├── lib.rs
│   ├── entrypoint.rs
│   ├── errors.rs
│   ├── instructions/{mod.rs, initialize.rs}
│   └── states/{mod.rs, state.rs, utils.rs}
├── tests/tests.rs
└── target/deploy/my_app-keypair.json
```

</details>

## Configuration

`Pinoc.toml` is optional. Every section but `[provider]` can be left out:

```toml
[provider]
cluster = "localhost"
wallet = "~/.config/solana/id.json"
program_keypair = "../keys/program.json"   # see Deploying
upgrade_authority = "..."                  # see Deploying

[build]
arch = "v3"                     # see SBPF version

[test]
features = ["test-default"]     # see Test configuration
programs = ["tests/mock_venue"]

[clusters.devnet]               # see Clusters
features = ["devnet"]

[idl]
generator = "auto"
out_dir = "idl"                 # see IDL and client generation

[client]
languages = ["ts", "rust"]
generator = "codama"

[check]                         # see src/check/README.md
authority_names = ["keeper"]
```

Without the file, `pinoc deploy` falls back to `solana config get`; `--cluster`/`--wallet` always override. Create one on demand with `pinoc config init`. A misspelt section, or a misspelt key under `[build]`, `[test]`, `[clusters.*]` or `[check]`, stops the command and names it. Releases before 0.4 refuse the `[test]` and `[clusters]` sections, so a project that uses them needs this pinoc everywhere it runs.

## Key management

```bash
pinoc keys list         # list program keypairs and their addresses
pinoc keys sync         # rewrite the program ID in source to match the keypair
```

`keys sync` finds the program's own ID declaration anywhere under `src/` (either `declare_id!` or a `const ID`) and updates it in place.

## IDL and client generation

`pinoc build` regenerates the IDL at `target/idl/` (or `[idl] out_dir`) on every build, and `pinoc client generate` renders a standalone Rust client crate from it, or a TypeScript client with `--language ts`. Both understand shank programs and programs using native [Codama](https://github.com/codama-idl/codama) derive macros.

A project that commits its IDL and clients can keep them current in CI with no script: set `[idl] out_dir` and `[client] languages`, and run

```bash
pinoc idl --check               # the committed IDL matches the source
pinoc client generate --check   # every configured client matches the committed IDL
```

Both write nothing, list each stale file, and exit nonzero.

- IDL generation (files produced, generator selection, error handling, the zero-copy padding lint): [src/idl/README.md](src/idl/README.md)
- Client generation (the shank and Codama generators, CPI variants, `fetch_*` helpers, output paths): [src/client_gen/README.md](src/client_gen/README.md)

## Linting

`pinoc check` statically lints a program for Solana-specific safety issues that rustc, clippy, and rust-analyzer do not model: account ownership and signer checks, cross-program invocation safety, zero-copy memory layout, and discriminator constants that are not the hash they claim to be (`// pinoc:discriminators("event:", EventKind)`). Configurable severity, inline `// pinoc:allow(CODE)` suppression, and output for CI as JSON, GitHub annotations or SARIF (`--format json|github|sarif`). A run that finds no handlers to analyse says so (`NO-HANDLERS`) instead of reporting a clean result.

```bash
pinoc check                  # report findings, exit nonzero on a deny
pinoc check --deny '*'       # promote every check to a hard failure
pinoc check --format github  # annotate the pull request in GitHub Actions
pinoc check --explain ACC001-P   # what a code means, with a flagged and a clean example
```

- Lint codes, configuration, and suppression: [src/check/README.md](src/check/README.md)

## CI

From 0.4.0, release binaries are attached to each GitHub release, so CI does not need to compile pinoc. The repository is also a GitHub Action that installs a pinned version, and optionally the Solana CLI and SBF toolchain:

```yaml
- uses: a91y/pinoc@v0.4.0
  with:
    version: 0.4.0            # pin it: a different pinoc can write a different IDL
    solana-version: <x.y.z>   # optional: also install this Agave release (solana, cargo build-sbf)
- run: pinoc idl --check && pinoc client generate --check --auto-install
- run: pinoc check --format github   # annotates the pull request
- run: pinoc test --all-clusters   # needs solana-version
```

For a version with no prebuilt binary for the runner, the action falls back to `cargo install`. `cargo binstall pinoc --version <v>` does the same outside Actions.

## Prerequisites

- [Rust](https://rustup.rs/) 1.70+
- [Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools)
- Node.js/npm (only for the Codama client generator)

## Contributing

Fork, branch, make your change with tests, and open a pull request.

```bash
git clone https://github.com/A91y/pinoc.git
cd pinoc && cargo build --release && cargo install --path .
```

## License

Apache 2.0. See [LICENSE](LICENSE).

## Support

- Issues: [GitHub Issues](https://github.com/A91y/pinoc/issues)
- Discussions: [GitHub Discussions](https://github.com/A91y/pinoc/discussions)
- Pinocchio: [anza-xyz/pinocchio](https://github.com/anza-xyz/pinocchio)

## Acknowledgements

Pinoc began as a fork of [solana-chio](https://github.com/aarjn/solana-chio) by [Arjun](https://github.com/aarjn).
