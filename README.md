# chio 

<div align="center">
  <img src="assets/logo.png" alt="Bruno CLI Logo" width="20%">
</div>

A command-line tool to setup your pinocchio project blazingly fast

## Installation

```bash
cargo install --git https://github.com/4rjunc/solana-chio --force
```

## Build your CLI tool

```bash
cargo build --release
```

## Install your CLI tool globally

```bash
cargo install --path .
```

## After installation, you'll be able to run:

```bash
chio init my-project
```

TODO
- add testcase folder
    - pass user address to testcase file ✅
    - change the build file path
- create a good readme
- change the banner 
- add wrap to command like 
    - `chio build` -> `cargo build-sbf` # half done
    - `chio test` -> `cargo test --features test-default`
    - `chio deploy` -> `solana program deploy ./target/debug/<project_name>.so`
    - `chio bench` -> `cargo bench --features bench-default`
- refactor
