<div align="center">
  <img src="assets/logo.png" alt="Chio CLI Logo" width="20%">
  <h1>Chio CLI</h1>
  <p>Setup Solana Pinocchio projects blazingly fast</p>
</div>

## About

Chio is a command-line tool designed to make it easy to set up and manage [Pinocchio](https://github.com/solana-labs/pinocchio) projects on Solana. It automates common development tasks including project initialization, building, testing, and deployment with simple commands.

## Features

- 🚀 Fast project scaffolding with best practices
- 📁 Proper directory structure for Solana/Pinocchio development
- 🔨 Simple build, test, and deployment commands
- 💻 Comprehensive testing environment setup

## Installation

### From GitHub

```bash
cargo install --git https://github.com/4rjunc/solana-chio --force
```

### From Source

1. Clone the repository
   ```bash
   git clone https://github.com/4rjunc/solana-chio.git
   cd solana-chio
   ```

2. Build the tool
   ```bash
   cargo build --release
   ```

3. Install globally
   ```bash
   cargo install --path .
   ```

## Usage

### Available Commands

```bash
# Initialize a new project
chio init <project-name>

# Build your project
chio build

# Run tests
chio test

# Deploy your program
chio deploy

# Run benchmarks
chio bench

# Get help
chio --help
```

### Example

Create a new Pinocchio project and get started:

```bash
# Create a new project
chio init my-pinocchio-app

# Navigate to your project
cd my-pinocchio-app

# Build your project
chio build

# Run tests
chio test

# Run benchmarks
chio bench
```



## Project Structure

When you initialize a project with `chio init`, it creates the following structure:

```
my-project/
├── Cargo.toml
├── src/
│   ├── lib.rs               # Library crate using no_std
│   ├── entrypoint.rs        # Program entrypoint
│   ├── errors.rs            # Error definitions
│   ├── instructions/        # Program instructions
│   │   ├── mod.rs
│   │   ├── deposit.rs
│   │   └── withdraw.rs
│   └── states/              # Account state definitions
│       ├── mod.rs
│       └── utils.rs
└── tests/                   # Test files
    └── unit_tests.rs
```

## Development Roadmap

### Completed ✅
- Generate proper project structure
- Pass user address to testcase file
- Create a comprehensive README template
- Implement `chio build` command

### In Progress 🚧
- Implement remaining command wrappers:
  - `chio test` → `cargo test --features test-default`
  - `chio deploy` → `solana program deploy ./target/debug/<project_name>.so`
  - `chio bench` → `cargo bench --features bench-default`
- Update banner and styling

### Planned 📋
- Add more sophisticated testing templates
- Improve error handling
- Add configuration options
- proper gitbook for the project

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

1. Ensure you have Rust and Cargo installed
2. Install Solana CLI tools
3. Clone the repository
4. Build with `cargo build`
5. Run with `cargo run -- <command>`

## License

This project is licensed under the MIT License - see the LICENSE file for details.
