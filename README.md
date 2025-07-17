<div align="center"> <img src="assets/logo.png" alt="Pinoc CLI Logo" width="20%"> <h1>Pinoc</h1> <p>Setup Solana Pinocchio projects blazingly fast</p>

[![Crates.io](https://img.shields.io/crates/v/pinoc)](https://crates.io/crates/pinoc)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70+-blue.svg)](https://www.rust-lang.org)

**Authors:**

<a class="header-badge" target="_blank" href="https://twitter.com/AyushAgr91"> <img alt="Twitter" src="https://img.shields.io/badge/@AyushAgr91-000000?style=for-the-badge&logo=x&logoColor=white"> </a>
<a class="header-badge" target="_blank" href="https://twitter.com/4rjunc"> <img alt="Twitter" src="https://img.shields.io/badge/@4rjunc-000000?style=for-the-badge&logo=x&logoColor=white"> </a> </div>

## About

Pinoc is a command-line tool designed to make it easy to set up and manage [Pinocchio](https://github.com/anza-xyz/pinocchio) projects on Solana. It automates common development tasks including project initialization, building, testing, and deployment with simple commands.

## Features

- 🚀 Fast project scaffolding with best practices
- 📁 Proper directory structure for Solana/Pinocchio development
- 🔨 Simple build, test, and deployment commands
- 🧹 Smart project cleaning with keypair preservation
- 📦 Package management and search functionality
- 💻 Comprehensive testing environment setup
- 🔐 Automatic keypair generation and management

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

1. Clone the repository

   ```bash
   git clone https://github.com/a91y/pinoc.git
   cd pinoc
   ```

2. Build the tool

   ```bash
   cargo build --release
   ```

3. Install globally
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

```bash
# Initialize a new project
pinoc init <project-name>

# Build your project
pinoc build

# Run tests
pinoc test

# Deploy your program
pinoc deploy

# Clean target directory (preserves keypairs by default)
pinoc clean

# Clean target directory (removes everything including keypairs)
pinoc clean --no-preserve

# Add a package to your project
pinoc add <package-name>

# Search for Pinocchio packages
pinoc search [query]

# Get help
pinoc --help
```

### Example

Create a new Pinocchio project and get started:

```bash
# Create a new project
pinoc init my-pinocchio-app

# Navigate to your project
cd my-pinocchio-app

# Build your project
pinoc build

# Run tests
pinoc test

# Clean build artifacts (preserves keypairs)
pinoc clean

# Add a package
pinoc add some-package

# Search for packages
pinoc search database

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

1. **Prerequisites**
   - Rust and Cargo installed
   - Solana CLI tools installed
   - Git for version control

2. **Clone and Build**
   ```bash
   git clone https://github.com/a91y/pinoc.git
   cd pinoc
   cargo build --release
   ```

3. **Install Locally**
   ```bash
   cargo install --path .
   ```

4. **Test Your Changes**
   ```bash
   # Test the CLI
   pinoc --help
   
   # Create a test project
   pinoc init test-project
   cd test-project
   pinoc build
   ```
