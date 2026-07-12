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
| `pinoc help`           | Show help             | `pinoc help`                    |

### Command Options

- `pinoc init <name> --no-git` - Skip git initialization
- `pinoc init <name> --with-example` - Include a worked PDA-account example instead of a no-op program
- `pinoc clean --no-preserve` - Clean everything including keypairs
- `pinoc deploy --cluster <cluster> --wallet <path>` - Override deployment settings

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
