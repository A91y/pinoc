# Changelog

All notable changes to the `pinoc` CLI tool will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
