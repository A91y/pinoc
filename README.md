<div align="center">
  <img src="assets/logo.png" alt="Pinoc CLI Logo" width="20%">
  <h1>Pinoc</h1>
  <p><strong>Setup Solana Pinocchio projects blazingly fast ⚡</strong></p>

[![Crates.io](https://img.shields.io/crates/v/pinoc)](https://crates.io/crates/pinoc)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-yellow.svg)](https://opensource.org/licenses/Apache-2.0)
[![Rust](https://img.shields.io/badge/rust-1.70+-blue.svg)](https://www.rust-lang.org)
[![Downloads](https://img.shields.io/crates/d/pinoc)](https://crates.io/crates/pinoc)

**Built by:**

  <a class="header-badge" target="_blank" href="https://twitter.com/AyushAgr91">
    <img alt="Twitter" src="https://img.shields.io/badge/@AyushAgr91-000000?style=for-the-badge&logo=x&logoColor=white">
  </a>
  <a class="header-badge" target="_blank" href="https://twitter.com/4rjunc">
    <img alt="Twitter" src="https://img.shields.io/badge/@4rjunc-000000?style=for-the-badge&logo=x&logoColor=white">
  </a>
</div>

---

## 🚀 What is Pinoc?

A modern Rust CLI to bootstrap Solana [Pinocchio](https://github.com/anza-xyz/pinocchio) programs with built-in build, deploy, and testing tools.

### Why Pinoc?

- **Zero Configuration**: Get started in seconds with sensible defaults
- **Best Practices**: Project structure follows Solana development conventions
- **Developer Experience**: Intuitive commands that feel natural
- **Production Ready**: Built-in testing, deployment, and key management

## ✨ Key Features

- 🏗️ **Instant Project Scaffolding** - Create production-ready projects in seconds
- 📁 **Optimized Structure** - Best-practice directory layout out of the box
- 🔨 **Unified Commands** - Build, test, and deploy with simple commands
- 🧹 **Smart Cleaning** - Clean build artifacts while preserving keypairs
- 📦 **Package Discovery** - Find and add Pinocchio packages effortlessly
- 🧪 **Built-in Testing** - Comprehensive testing with mollusk-svm
- 🔐 **Keypair Management** - Automatic generation and secure storage
- 🔑 **Program ID Sync** - Keep your program IDs consistent automatically
- ⚙️ **Configuration Management** - Simple deployment configuration with Pinoc.toml

## 📦 Installation

### Quick Install (Recommended)

```bash
cargo install pinoc
```

### Alternative Methods

<details>
<summary>From GitHub (Latest)</summary>

```bash
cargo install --git https://github.com/a91y/pinoc --force
```

</details>

<details>
<summary>From Source</summary>

```bash
git clone https://github.com/a91y/pinoc.git
cd pinoc
cargo build --release
cargo install --path .
```

</details>

## 🎯 Quick Start

```bash
# Install pinoc
cargo install pinoc

# Create a new project
pinoc init my_awesome_app

# Navigate to your project
cd my_awesome_app

# Build and test
pinoc build
pinoc test

# Deploy to Solana
pinoc deploy
```

That's it! You now have a fully functional Solana program ready for development.

## 📋 Command Reference

| Command                | Description           | Example                         |
| ---------------------- | --------------------- | ------------------------------- |
| `pinoc init <name>`    | Create a new project  | `pinoc init my_app`             |
| `pinoc build`          | Build your program    | `pinoc build`                   |
| `pinoc test`           | Run tests             | `pinoc test`                    |
| `pinoc deploy`         | Deploy to Solana      | `pinoc deploy --cluster devnet` |
| `pinoc clean`          | Clean build artifacts | `pinoc clean`                   |
| `pinoc add <package>`  | Add a package         | `pinoc add some_package`        |
| `pinoc search [query]` | Search packages       | `pinoc search database`         |
| `pinoc keys list`      | List program keypairs | `pinoc keys list`               |
| `pinoc keys sync`      | Sync program IDs      | `pinoc keys sync`               |
| `pinoc idl`            | Regenerate the IDL JSON | `pinoc idl --out-dir target/idl` |
| `pinoc client generate` | Generate a Rust client from the IDL | `pinoc client generate` |
| `pinoc config init`    | Create a Pinoc.toml for this project | `pinoc config init`    |
| `pinoc help`           | Show help             | `pinoc help`                    |

### Command Options

- `pinoc init <name> --no-git` - Skip git initialization
- `pinoc init <name> --with-example` - Include a worked PDA-account example instead of a no-op program
- `pinoc clean --no-preserve` - Clean everything including keypairs
- `pinoc deploy --cluster <cluster> --wallet <path>` - Override deployment settings
- `pinoc build --program-id <ADDRESS>` - Override the program address used in IDL generation, for programs that don't call `declare_id!`

## 📂 Project Structure

### Standard Project

A blank, buildable program with a single no-op instruction, ready for you to fill in:

```
my_minimal_project/
├── Cargo.toml              # Project configuration
├── README.md               # Basic documentation
├── .gitignore              # Git ignore rules
├── Pinoc.toml              # Deployment configuration
├── src/
│   └── lib.rs              # Minimal program structure
└── target/deploy/
    └── my_minimal_project-keypair.json
```

### Worked Example Project (`--with-example`)

A full PDA-account creation example, useful as a reference for the CPI/PDA pattern:

```
my_project/
├── Cargo.toml              # Project configuration
├── README.md               # Documentation
├── .gitignore              # Git ignore rules
├── Pinoc.toml              # Deployment configuration
├── src/
│   ├── lib.rs              # Main library
│   ├── entrypoint.rs       # Program entrypoint
│   ├── errors.rs           # Error definitions
│   ├── instructions/       # Program instructions
│   │   ├── mod.rs
│   │   └── initialize.rs
│   └── states/             # Account states
│       ├── mod.rs
│       ├── state.rs
│       └── utils.rs
├── tests/
│   └── tests.rs            # Unit tests
└── target/deploy/
    └── my_project-keypair.json  # Program keypair
```

## 🔧 Advanced Usage

### Configuration Management

Pinoc uses `Pinoc.toml` for deployment settings:

```toml
[provider]
cluster = "localhost"
wallet = "~/.config/solana/id.json"
```

`Pinoc.toml` isn't required for `pinoc deploy` — if it's missing, cluster/wallet defaults come from `solana config get` (your Solana CLI's own config) instead. `--cluster`/`--wallet` always override individual fields regardless of where the defaults came from. Any command that would've used `Pinoc.toml` (deploy, IDL generation) still works without one; it just prints a hint pointing at `pinoc config init` in case you want one.

Create a `Pinoc.toml` for an existing project on demand:

```bash
pinoc config init       # refuses (with a hint) if this doesn't look like a Pinocchio project
pinoc config init -y    # skip that check
```

Override settings per deployment:

```bash
# Deploy to devnet with custom wallet
pinoc deploy --cluster devnet --wallet ./custom-keypair.json
```

### Key Management

Keep your program IDs synchronized:

```bash
# Check key consistency
pinoc keys list

# Sync program ID in lib.rs with keypair
pinoc keys sync
```

Example output:

```
✅ Program key is already consistent!
🔑 Program ID: 9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM
📝 No update needed in src/lib.rs
```

### Smart Cleaning

Clean build artifacts while preserving important files:

```bash
# Clean target directory (preserves keypairs)
pinoc clean

# Clean everything including keypairs
pinoc clean --no-preserve
```

### IDL Generation

`pinoc build` automatically (re)generates two IDL files at `target/idl/`, describing your program's instructions, accounts, types, and errors — powered by [shank](https://github.com/metaplex-foundation/shank), built into `pinoc` itself (no separate install needed). A failed IDL extraction only prints a warning; it never fails the build.

- `<project_name>.json` — shank's native IDL, exactly as extracted. This is what `pinoc client generate` reads.
- `<project_name>.codama.json` — the same IDL with pubkey-typed fields rewritten from shank's `{"defined": "Address"}` to the standard `"publicKey"` IDL type. Use this one if you want to feed the IDL into a real [Codama](https://github.com/codama-idl/codama) (Node.js) pipeline yourself — shank's native representation trips a bug in `@codama/nodes-from-anchor`'s conversion (see [docs/codama-comparison.md](docs/codama-comparison.md)); this file sidesteps it.

You can also regenerate both on their own:

```bash
pinoc idl
pinoc idl --out-dir custom_idl_dir
pinoc idl --program-id <ADDRESS>          # for programs that don't call declare_id!
pinoc build --program-id <ADDRESS>        # same override, for the automatic post-build IDL step
```

The `--with-example` project's instructions and accounts are already annotated so `pinoc idl` works out of the box. Programs that declare their program ID via `Address::from_str_const` instead of `declare_id!` (a common pattern to avoid the extra `pinocchio-pubkey`/`decode`-feature dependency) need `--program-id` since shank can't find the address in the source otherwise — everything else about the program (instructions, accounts, errors) still needs shank's derive macros to show up in the IDL; `pinoc idl` doesn't infer them from unannotated code.

Errors are the one exception: if your error enum doesn't derive `thiserror::Error` (all `shank_idl` recognizes on its own), `pinoc idl` falls back to detecting a plain enum with a manual `impl From<X> for ProgramError`, synthesizing each message from the variant name (`InvalidPda` -> "Invalid Pda").

#### Native Codama macros

If your program is annotated with [Codama](https://github.com/codama-idl/codama-rs)'s own Rust derive macros (`CodamaAccount`, `CodamaInstructions`, `CodamaErrors`, `CodamaType`, etc. — a `codama` dependency plus at least one of these derives) `pinoc idl` detects it automatically and generates `.codama.json` from Codama's own extractor instead of the shank+shim path. You can tell which one you got: native output has a top-level `"kind": "rootNode"` field; shim output doesn't (it carries `"metadata": {"origin": "shank", ...}` instead). Either way, the plain `<name>.json` is unaffected — it's always shank's extraction.

Override the choice per-invocation or per-project:

```bash
pinoc idl --idl-generator shank     # force the shim even if Codama macros are present
pinoc idl --idl-generator codama    # force native extraction (errors if no Codama macros exist)
```

```toml
# Pinoc.toml
[idl]
generator = "auto"   # "auto" | "shank" | "codama"
```
CLI flag wins over `Pinoc.toml`, which wins over auto-detection.

### Client Generation

`pinoc client generate` writes a small, standalone Rust client crate to `clients/rust-shank/` or `clients/rust-codama/` (the default output directory depends on the resolved generator, so running both doesn't overwrite one with the other; override with `--out-dir`). Two generators are available:

- **`shank`** (recommended unless Codama macros are detected, see below) — pure Rust, built into `pinoc`, no Node.js/npm required. Reads `target/idl/<name>.json`. Borsh-based instruction builders, account (de)serialization, `declare_id!` matching your program's address, plus CPI variants and `fetch_*` RPC helpers (see below). Styled after Codama's Rust renderer conventions, though not literally Codama's own renderer output.
- **`codama`** — shells out to the real [Codama](https://github.com/codama-idl/codama) JS pipeline (`@codama/nodes-from-anchor` + `@codama/renderers-rust`). Reads `target/idl/<name>.codama.json`. Requires Node.js/npm.

**CPI variants**: `pinoc client generate` scans your program's own source for `invoke`/`invoke_signed` call sites and, if found, adds `XxxCpi`/`XxxCpiAccounts`/`XxxCpiBuilder` for each instruction (for calling this program from another program via cross-program invocation), plus the 3 extra dependencies they need (`solana-account-info`, `solana-cpi`, `solana-program-error`). Override the detection:

```bash
pinoc client generate --with-cpi   # force CPI variants even if not detected
pinoc client generate --no-cpi     # never generate them, regardless of detection
```

**`fetch_*` RPC helpers**: always generated (`fetch_x`/`fetch_all_x`/`fetch_maybe_x`/`fetch_all_maybe_x` per account), but gated behind a `fetch` Cargo feature in the generated client so `solana-rpc-client`/`solana-account` are only pulled in when you actually opt in: `cargo build --features fetch` in the generated client crate.

```bash
pinoc client generate                              # prompts to choose shank/codama if run interactively
pinoc client generate --generator shank
pinoc client generate --generator codama --auto-install   # installs codama's npm deps on first run
pinoc client generate --out-dir clients/rust --idl-dir target/idl   # custom path, shared by both if you want
```

`--out-dir` can also be set persistently in `Pinoc.toml` (not added by default in scaffolded projects; only the `[provider]`/`[idl]` sections are):

```toml
[client]
out_dir = "clients/rust"        # shared by both generators; using it prints a warning every time, since switching generators overwrites the other's output
shank_out_dir = "clients/shank" # per-generator path, wins over the shared out_dir above
codama_out_dir = "clients/codama"
```

Precedence: `--out-dir` (CLI) > `shank_out_dir`/`codama_out_dir` > `out_dir` (shared) > the dynamic default (`clients/rust-shank`/`clients/rust-codama`) if none of the above are set.

If you pick `codama` and its npm dependencies (managed in a project-local `<out-dir>/.pinoc-codama/`, isolated from anything else) aren't installed yet, `pinoc` stops and shows the exact `npm install` command to run — it won't install anything without your say-so unless you pass `--auto-install`. If Node.js itself isn't found, it prints a quick-install pointer instead. `.pinoc-codama/` is added to your project's `.gitignore` automatically (a new one is only created if the project is a git repo).

`pinoc client generate` uses the same Codama-macro detection as `pinoc idl` to recommend a generator: `codama` if your program has Codama macros, `shank` otherwise. When run interactively without `--generator`, the prompt marks whichever one is recommended and that's what bare Enter picks. If you pass `--generator` explicitly and it contradicts the recommendation (e.g. `--generator shank` on a program with Codama macros), you'll be asked to confirm; skip that with `-y`/`--yes`. Non-interactively (no TTY) without `-y`, it refuses and tells you to add the flag rather than guessing.

Run `pinoc build` (or `pinoc idl`) first so the IDL exists. The generated crate is standalone — `cd clients/rust-shank && cargo build` (or `clients/rust-codama`) — and not part of the program's own Cargo workspace.

## 🔗 Prerequisites

Ensure you have these tools installed:

- **Rust** (1.70+) - [Install here](https://rustup.rs/)
- **Solana CLI** - [Install guide](https://docs.solana.com/cli/install-solana-cli-tools)
- **Git** - For version control

## 🤝 Contributing

We welcome contributions! Here's how to get started:

1. **Fork** the repository
2. **Create** a feature branch (`git checkout -b feature/amazing-feature`)
3. **Make** your changes
4. **Test** thoroughly
5. **Commit** with clear messages (`git commit -m 'Add amazing feature'`)
6. **Push** to your branch (`git push origin feature/amazing-feature`)
7. **Open** a Pull Request

### Development Setup

```bash
git clone https://github.com/a91y/pinoc.git
cd pinoc
cargo build --release
cargo install --path .

# Test your changes
pinoc init test-project
cd test-project
pinoc build
```

## 📄 License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## 🆘 Support & Community

- 📖 **Documentation**: [Pinocchio Docs](https://github.com/anza-xyz/pinocchio)
- 🐛 **Issues**: [GitHub Issues](https://github.com/a91y/pinoc/issues)
- 💬 **Discussions**: [GitHub Discussions](https://github.com/a91y/pinoc/discussions)
- 📦 **Crates.io**: [pinoc](https://crates.io/crates/pinoc)

---

<div align="center">
  <p>Made with ❤️ by the Solana community</p>
  <p>⭐ Star us on GitHub if Pinoc helps you build faster!</p>
</div>
