pub mod instructions;
pub mod minimal;
pub mod states;
pub mod unit_tests;

//lib.rs
pub fn lib_rs(address: &str) -> String {
    format!(
        r#"#![no_std]

#[cfg(not(feature = "no-entrypoint"))]
mod entrypoint;

#[cfg(feature = "std")]
extern crate std;

pub mod errors;
pub mod instructions;
pub mod states;

pinocchio_pubkey::declare_id!("{}");"#,
        address
    )
}

// entrypoint.rs template
pub fn entrypoint_rs() -> &'static str {
    r#"#![allow(unexpected_cfgs)]

use crate::instructions::{self, ProgramInstruction};
use pinocchio::{
    account_info::AccountInfo, default_panic_handler, msg, no_allocator, program_entrypoint,
    program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

// This is the entrypoint for the program.
program_entrypoint!(process_instruction);
//Do not allocate memory.
no_allocator!();
// Use the no_std panic handler.
default_panic_handler!();

#[inline(always)]
fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let (ix_disc, instruction_data) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;

    match ProgramInstruction::try_from(ix_disc)? {
        ProgramInstruction::InitializeState => {
            msg!("initialize");
            instructions::initialize(accounts, instruction_data)
        }
    }
}"#
}

// Configuration files
pub fn readme_md() -> &'static str {
    r#"# Pinoc Pinocchio Project

A Solana program built with the Pinoc CLI tool.

## Project Structure

```
src/
├── entrypoint.rs          # Program entry point with nostd_panic_handler
├── lib.rs                 # Library crate (no_std optimization)
├── instructions/          # Program instruction handlers  
├── states/                # Account state definitions
│   └── utils.rs           # State management helpers (load_acc, load_mut_acc)
└── errors.rs              # Program error definitions

tests/
└── tests.rs               # Unit tests using mollusk-svm framework
```

## Commands

```bash
# Build the program
pinoc build

# Run tests
pinoc test

# Deploy the program
pinoc deploy

# Get help
pinoc help
```

---

**Author of Pinoc CLI**: [4rjunc](https://github.com/4rjunc) | [Twitter](https://x.com/4rjunc)"#
}

pub fn gitignore() -> &'static str {
    r#"/target
.env"#
}

pub fn pinoc_toml() -> &'static str {
    r#"[provider]
cluster = "localhost"
wallet = "~/.config/solana/id.json"
"#
}

pub fn errors_rs() -> &'static str {
    r#"use pinocchio::program_error::ProgramError;

#[derive(Clone, PartialEq, shank::ShankType)]
pub enum MyProgramError {
    InvalidInstructionData,
    PdaMismatch,
    InvalidOwner,
}

impl From<MyProgramError> for ProgramError {
    fn from(e: MyProgramError) -> Self {
        Self::Custom(e as u32)
    }
}       
"#
}

pub fn cargo_toml(project_name: &str) -> String {
    format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
pinocchio = "0.8.4"
pinocchio-log = "0.4.0"
pinocchio-pubkey = "0.2.4"
pinocchio-system = "0.2.3"
shank = "0.4.2"

[dev-dependencies]
solana-sdk = "2.3.0"
solana-program-runtime = "=2.3.1"
mollusk-svm = "0.3.0"
mollusk-svm-bencher = "0.3.0" 

[features]
no-entrypoint = []
std = []
test-default = ["no-entrypoint", "std"]
    "#,
        project_name
    )
}
