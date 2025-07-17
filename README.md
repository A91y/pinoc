<div align="center">
  <img src="assets/logo.png" alt="Pinoc CLI Logo" width="20%">
  <h1>Pinoc</h1>
  <p>Setup Solana Pinocchio projects blazingly fast</p>

  [![Crates.io](https://img.shields.io/crates/v/pinoc)](https://crates.io/crates/pinoc)
  [![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
  [![Rust](https://img.shields.io/badge/rust-1.70+-blue.svg)](https://www.rust-lang.org)

  **Authors:**

  <a class="header-badge" target="_blank" href="https://twitter.com/AyushAgr91">
    <img alt="Twitter" src="https://img.shields.io/badge/@AyushAgr91-000000?style=for-the-badge&logo=x&logoColor=white">
  </a>
  <a class="header-badge" target="_blank" href="https://twitter.com/4rjunc">
    <img alt="Twitter" src="https://img.shields.io/badge/@4rjunc-000000?style=for-the-badge&logo=x&logoColor=white">
  </a>
</div>

## About

Pinoc is a command-line tool designed to make it easy to set up and manage [Pinocchio](https://github.com/anza-xyz/pinocchio) projects on Solana. It automates common development tasks including project initialization, building, testing, and deployment with simple commands.

## Features

- 🚀 **Fast Project Scaffolding** - Create new projects with best practices in seconds
- 📁 **Proper Directory Structure** - Solana/Pinocchio development structure out of the box
- 🔨 **Simple Commands** - Build, test, and deploy with intuitive commands
- 🧹 **Smart Project Cleaning** - Clean build artifacts while preserving keypairs
- 📦 **Package Management** - Add dependencies and search for Pinocchio packages
- 💻 **Comprehensive Testing** - Built-in testing environment with mollusk-svm
- 🔐 **Automatic Keypair Management** - Generate and manage program keypairs
- 🔑 **Program Key Sync** - Keep your program IDs consistent with smart checking

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

1. **Clone the repository**
   ```bash
   git clone https://github.com/a91y/pinoc.git
   cd pinoc
   ```

2. **Build the tool**
   ```bash
   cargo build --release
   ```

3. **Install globally**
   ```bash
   cargo install --path .
   ```

## Quick Start

```bash
# Install pinoc
cargo install pinoc

# Create a new project
pinoc init my-awesome-app

# Navigate to your project
cd my-awesome-app

# Build and test
pinoc build
pinoc test

# Deploy to Solana
pinoc deploy
```

## Usage

### Available Commands

| Command | Description |
|---------|-------------|
| `pinoc init <project-name>` | Initialize a new Pinocchio project |
| `pinoc build` | Build your Solana program |
| `pinoc test` | Run project tests |
| `pinoc deploy` | Deploy your program to Solana |
| `pinoc clean [--no-preserve]` | Clean target directory (preserves keypairs by default) |
| `pinoc add <package-name>` | Add a package to your project |
| `pinoc search [query]` | Search for Pinocchio packages |
| `pinoc keys list` | List all program keypairs |
| `pinoc keys sync` | Sync program ID in lib.rs with keypair |
| `pinoc --help` | Get help and see all available commands |

### Complete Workflow Example

```bash
# Create a new project
pinoc init my-pinocchio-app

# Navigate to your project
cd my-pinocchio-app

# Build your project
pinoc build

# Run tests
pinoc test

# List program keys
pinoc keys list

# Sync program keys (checks consistency first)
pinoc keys sync

# Clean build artifacts (preserves keypairs)
pinoc clean

# Add a package
pinoc add some-package

# Search for packages
pinoc search database

# Deploy your program
pinoc deploy
```

## Project Structure

When you initialize a project with `pinoc init`, it creates the following structure:

```
my-project/
├── Cargo.toml              # Project configuration with Pinocchio dependencies
├── README.md               # Project documentation
├── .gitignore              # Git ignore file
├── src/
│   ├── lib.rs              # Library crate using no_std
│   ├── entrypoint.rs       # Program entrypoint
│   ├── errors.rs           # Error definitions
│   ├── instructions/       # Program instructions
│   │   ├── mod.rs
│   │   └── initialize.rs   # Initialize instruction
│   └── states/             # Account state definitions
│       ├── mod.rs
│       ├── state.rs        # State structure
│       └── utils.rs        # State management utilities
├── tests/                  # Test files
│   └── tests.rs            # Unit tests using mollusk-svm
└── target/
    └── deploy/
        └── my-project-keypair.json  # Generated program keypair
```

## Key Features in Detail

### 🧹 Smart Project Cleaning

The `pinoc clean` command intelligently manages your build artifacts:

```bash
# Clean target directory while preserving keypairs (default)
pinoc clean

# Clean everything including keypairs
pinoc clean --no-preserve
```

**Why preserve keypairs?** Your program keypair is essential for deployment. The default behavior ensures you don't accidentally lose your deployment credentials.

### 📦 Package Management

Easily add dependencies and discover new packages:

```bash
# Add a package to your project
pinoc add package-name

# Search for Pinocchio-related packages
pinoc search database
pinoc search oracle
```

### 🔐 Automatic Keypair Management

- **Generation**: Keypairs are automatically created during project initialization
- **Preservation**: Clean commands preserve keypairs by default
- **Security**: Keypairs are stored securely in `target/deploy/`

### 🔑 Program Key Management

Manage your program keys with consistency checks:

```bash
# List all program keypairs
pinoc keys list

# Sync program ID in lib.rs with keypair
pinoc keys sync
```

**Key Sync Features:**

- **Consistency Check**: Verifies if the program ID in `declare_id!` matches the keypair's public key
- **Smart Updates**: Only updates the file if there's a mismatch
- **Clear Feedback**: Shows current state and any changes made
- **No Unnecessary Writes**: Prevents file updates when keys are already consistent

**Example Output:**

```bash
# When keys are already consistent
✅ Program key is already consistent!
🔑 Program ID: 9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM
📝 No update needed in src/lib.rs

# When keys need syncing
🔄 Program key mismatch detected:
   Current in lib.rs: 11111111111111111111111111111111
   Actual keypair:    9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM
✅ Successfully synced program key!
🔑 Program ID: 9WzDXwBbmkg8ZTbNMqUxvQRAyrZzDsGYdLVL9zYtAWWM
📝 Updated src/lib.rs with new program ID
```

## Prerequisites

Before using Pinoc, make sure you have the following installed:

- **Rust and Cargo** - [Install Rust](https://rustup.rs/)
- **Solana CLI Tools** - [Install Solana CLI](https://docs.solana.com/cli/install-solana-cli-tools)
- **Git** - For version control

## Contributing

Contributions are welcome! Here's how you can contribute:

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Run tests to ensure everything works
5. Commit your changes (`git commit -m 'Add some amazing feature'`)
6. Push to the branch (`git push origin feature/amazing-feature`)
7. Open a Pull Request

### Development Setup

1. **Clone and Build**
   ```bash
   git clone https://github.com/a91y/pinoc.git
   cd pinoc
   cargo build --release
   ```

2. **Install Locally**
   ```bash
   cargo install --path .
   ```

3. **Test Your Changes**
   ```bash
   # Test the CLI
   pinoc --help

   # Create a test project
   pinoc init test-project
   cd test-project
   pinoc build
   ```

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.

## Support

- 📖 **Documentation**: This README and the [Pinocchio documentation](https://github.com/anza-xyz/pinocchio)
- 🐛 **Issues**: Report bugs and request features on [GitHub Issues](https://github.com/a91y/pinoc/issues)
