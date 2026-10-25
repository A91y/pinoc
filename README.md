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
pinoc verify            # check the deployed bytes are what you built
```

`Pinoc.toml` is optional: every command has defaults, and the file only records what differs ([Configuration](docs/configuration.md)).

## Commands

| Command | Does | Docs |
| --- | --- | --- |
| `pinoc init <name>` | Create a project (`--with-example` for a worked PDA program) | [Projects](docs/build-and-test.md#creating-a-project) |
| `pinoc build` | Build the program and regenerate the IDL | [Building](docs/build-and-test.md#building) |
| `pinoc test` | Build the program, then run the tests | [Testing](docs/build-and-test.md#testing) |
| `pinoc check` | Lint for Solana-specific safety issues | [`pinoc check`](docs/check.md) |
| `pinoc deploy` | Deploy to a cluster, at the address the program declares | [Deploying](docs/deploy.md#deploying) |
| `pinoc verify` | Compare the deployed program with the local artifact | [Verifying](docs/deploy.md#verifying-a-deploy) |
| `pinoc idl` | Regenerate the IDL (`--check` for CI) | [IDL](docs/idl.md) |
| `pinoc client generate` | Generate a Rust or TypeScript client (`--check` for CI) | [Clients](docs/clients.md) |
| `pinoc keys list` / `sync` | List program keypairs; sync the program ID in source | [Keys](docs/deploy.md#keys) |
| `pinoc config init` | Create a `Pinoc.toml` | [Configuration](docs/configuration.md) |
| `pinoc add` / `search` | Add or find a Pinocchio package | [Projects](docs/build-and-test.md#creating-a-project) |
| `pinoc clean` | Remove build artifacts, keeping keypairs | [Projects](docs/build-and-test.md#creating-a-project) |

`pinoc <command> --help` lists every flag.

## Documentation

- [Projects, building and testing](docs/build-and-test.md): features, `[test]`, the build record, SBPF versions
- [Clusters, deploying and keys](docs/deploy.md): per-cluster builds, the program keypair, `pinoc verify`
- [IDL generation](docs/idl.md) and [client generation](docs/clients.md): shank and Codama programs, TypeScript
- [`pinoc check`](docs/check.md) and [the lint codes](docs/lints/): what is checked, configuration, output formats
- [CI](docs/ci.md): the [setup action](https://github.com/marketplace/actions/setup-pinoc), prebuilt binaries, a workflow
- [Configuration](docs/configuration.md): every `Pinoc.toml` key

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
