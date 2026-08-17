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
| `pinoc test` | Run tests |
| `pinoc check` | Lint for Solana-specific safety issues (account, CPI, zero-copy) |
| `pinoc deploy` | Deploy to a cluster |
| `pinoc clean` | Clean build artifacts (keypairs preserved) |
| `pinoc add <package>` | Add a Pinocchio package |
| `pinoc search [query]` | Search packages |
| `pinoc keys list` | List program keypairs |
| `pinoc keys sync` | Sync the program ID in source with its keypair |
| `pinoc idl` | Regenerate the IDL JSON |
| `pinoc client generate` | Generate a Rust client from the IDL |
| `pinoc config init` | Create a `Pinoc.toml` for the project |

Common options:

- `pinoc init <name> --with-example`: scaffold a worked PDA-account example instead of a no-op program
- `pinoc init <name> --no-git`: skip git initialization
- `pinoc deploy --cluster <cluster> --wallet <path>`: override deployment settings
- `pinoc build --program-id <ADDRESS>`: set the IDL program address for programs that don't call `declare_id!`
- `pinoc clean --no-preserve`: clean everything, including keypairs

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

`Pinoc.toml` holds deployment defaults and is optional:

```toml
[provider]
cluster = "localhost"
wallet = "~/.config/solana/id.json"
```

Without it, `pinoc deploy` falls back to `solana config get`; `--cluster`/`--wallet` always override. Create one on demand with `pinoc config init`.

## Key management

```bash
pinoc keys list         # list program keypairs and their addresses
pinoc keys sync         # rewrite the program ID in source to match the keypair
```

`keys sync` finds the program's own ID declaration anywhere under `src/` (either `declare_id!` or a `const ID`) and updates it in place.

## IDL and client generation

`pinoc build` regenerates the IDL at `target/idl/` on every build, and `pinoc client generate` renders a standalone Rust client crate from it. Both understand shank programs and programs using native [Codama](https://github.com/codama-idl/codama) derive macros.

- IDL generation (files produced, generator selection, error handling, the zero-copy padding lint): [src/idl/README.md](src/idl/README.md)
- Client generation (the shank and Codama generators, CPI variants, `fetch_*` helpers, output paths): [src/client_gen/README.md](src/client_gen/README.md)

## Linting

`pinoc check` statically lints a program for Solana-specific safety issues that rustc, clippy, and rust-analyzer do not model: account ownership and signer checks, cross-program invocation safety, and zero-copy memory layout. Configurable severity, inline `// pinoc:allow(CODE)` suppression, and `--json` output for CI.

```bash
pinoc check                  # report findings, exit nonzero on a deny
pinoc check --deny '*'       # promote every check to a hard failure
```

- Lint codes, configuration, and suppression: [src/check/README.md](src/check/README.md)

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
