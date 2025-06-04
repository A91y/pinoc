pub mod templates {

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
            msg!("Initilaize");
            instructions::initilaize(accounts, instruction_data)
        }
    }
}"#
    }

    // Configuration files
    pub fn readme_md() -> &'static str {
        r#"# Chio Pinocchio Project
A project created with the Chio CLI tool.

## Getting Started

### 1. Project Structure

- **`src/`** - Source code folder
  - **`entrypoint.rs`** - Program entry point
    - Uses `nostd_panic_handler` for panic handling
    - Disables global allocator (no heap allocations)
  - **`lib.rs`** - Library crate
    - Uses `no_std` for performance optimization
  - **`instructions/`** - Contains all program instructions
  - **`states/`** - Contains all account state definitions
    - **`utils.rs`** - Helper functions for state management
      - Provides serialization/deserialization helpers (`load_acc`, `load_mut_acc`, etc.)
  - **`errors.rs`** - Program error definitions

- **`tests/`** - Test files
  - Uses `mollusk-svm` - A lightweight Solana testing framework
  - **`unit_tests.rs`** - Unit tests for the program

### 2. Common Commands

```bash
# Build the program
chio build

# Run tests
chio test

# Deploy the program
chio deploy

# Get help information
chio help
```

### 3. After Building

After a successful build, get the program public key:

```bash
solana address -k target/deploy/<YOUR_PROJECT_NAME>-keypair.json
```

Then replace the ID in your code:
```rust
pinocchio_pubkey::declare_id!("YourProgramIdHere");
```
"#
    }

    pub fn gitignore() -> &'static str {
        r#"/target
.env"#
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

    pub mod instructions {
        pub fn initilaize() -> &'static str {
            r#"use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::rent::Rent,
    ProgramResult,
};

use pinocchio_system::instructions::CreateAccount;

use crate::{
    errors::MyProgramError,
    states::{
        utils::{load_ix_data, DataLen},
        MyState,
    },
};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Initialize {
    pub owner: Pubkey,
    pub bump: u8,
}

impl DataLen for Initialize {
    const LEN: usize = core::mem::size_of::<Initialize>();
}

pub fn initilaize(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let [payer_acc, state_acc, sysvar_rent_acc, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !payer_acc.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if !state_acc.data_is_empty() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    let rent = Rent::from_account_info(sysvar_rent_acc)?;

    let ix_data = unsafe { load_ix_data::<Initialize>(data)? };

    if ix_data.owner.ne(payer_acc.key()) {
        return Err(MyProgramError::InvalidOwner.into());
    }

    let pda_bump_bytes = [ix_data.bump];

    MyState::validate_pda(ix_data.bump, state_acc.key(), &ix_data.owner)?;

    // signer seeds
    let signer_seeds = [
        Seed::from(MyState::SEED.as_bytes()),
        Seed::from(&ix_data.owner),
        Seed::from(&pda_bump_bytes[..]),
    ];
    let signers = [Signer::from(&signer_seeds[..])];

    CreateAccount {
        from: payer_acc,
        to: state_acc,
        space: MyState::LEN as u64,
        owner: &crate::ID,
        lamports: rent.minimum_balance(MyState::LEN),
    }
    .invoke_signed(&signers)?;

    MyState::initialize(state_acc, ix_data)?;

    Ok(())
}"#
        }

        pub fn instructions_mod_rs() -> &'static str {
            r#"use pinocchio::program_error::ProgramError;

pub mod initialize;

pub use initialize::*;

#[repr(u8)]
pub enum ProgramInstruction {
    InitializeState,
}

impl TryFrom<&u8> for ProgramInstruction {
    type Error = ProgramError;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        match *value {
            0 => Ok(ProgramInstruction::InitializeState),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}"#
        }
    }

    pub mod states {
        pub fn states_mod_rs() -> &'static str {
            r#"pub mod state;
pub mod utils;

pub use state::*;
pub use utils::*;"#
        }

        pub fn state_rs() -> &'static str {
            r#"use super::utils::{load_acc_mut_unchecked, DataLen};
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    ProgramResult,
};

use crate::{errors::MyProgramError, instructions::Initialize};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MyState {
    pub owner: Pubkey,
}

impl DataLen for MyState {
    const LEN: usize = core::mem::size_of::<MyState>();
}

impl MyState {
    pub const SEED: &'static str = "init";

    pub fn validate_pda(bump: u8, pda: &Pubkey, owner: &Pubkey) -> Result<(), ProgramError> {
        let seed_with_bump = &[Self::SEED.as_bytes(), owner, &[bump]];
        let derived = pubkey::create_program_address(seed_with_bump, &crate::ID)?;
        if derived != *pda {
            return Err(MyProgramError::PdaMismatch.into());
        }
        Ok(())
    }

    pub fn initialize(my_stata_acc: &AccountInfo, ix_data: &Initialize) -> ProgramResult {
        let my_state =
            unsafe { load_acc_mut_unchecked::<MyState>(my_stata_acc.borrow_mut_data_unchecked()) }?;

        my_state.owner = ix_data.owner;
        Ok(())
    }
}"#
        }

        pub fn utils_rs() -> &'static str {
            r#"use pinocchio::program_error::ProgramError;

use crate::errors::MyProgramError;

pub trait DataLen {
    const LEN: usize;
}

#[inline(always)]
pub unsafe fn load_acc_unchecked<T: DataLen>(bytes: &[u8]) -> Result<&T, ProgramError> {
    if bytes.len() != T::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(&*(bytes.as_ptr() as *const T))
}

#[inline(always)]
pub unsafe fn load_acc_mut_unchecked<T: DataLen>(bytes: &mut [u8]) -> Result<&mut T, ProgramError> {
    if bytes.len() != T::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(&mut *(bytes.as_mut_ptr() as *mut T))
}

#[inline(always)]
pub unsafe fn load_ix_data<T: DataLen>(bytes: &[u8]) -> Result<&T, ProgramError> {
    if bytes.len() != T::LEN {
        return Err(MyProgramError::InvalidInstructionData.into());
    }
    Ok(&*(bytes.as_ptr() as *const T))
}

pub unsafe fn to_bytes<T: DataLen>(data: &T) -> &[u8] {
    core::slice::from_raw_parts(data as *const T as *const u8, T::LEN)
}

pub unsafe fn to_mut_bytes<T: DataLen>(data: &mut T) -> &mut [u8] {
    core::slice::from_raw_parts_mut(data as *mut T as *mut u8, T::LEN)
}"#
        }
    }

    pub mod unit_tests {
        pub fn unit_test_rs(address: &str) -> String {
            let template = r#"
use mollusk_svm::result::{Check, ProgramResult};
use mollusk_svm::{program, Mollusk};
use solana_sdk::account::Account;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;
extern crate alloc;
use alloc::vec;

use solana_pinocchio_starter::instruction::Initialize;
use solana_pinocchio_starter::state::{to_bytes, DataLen, MyState};
use solana_pinocchio_starter::ID;
use solana_sdk::rent::Rent;
use solana_sdk::sysvar::Sysvar;

pub const PROGRAM: Pubkey = Pubkey::new_from_array(ID);

pub const RENT: Pubkey = pubkey!("SysvarRent111111111111111111111111111111111");

pub const PAYER: Pubkey = pubkey!("{address}");

pub fn mollusk() -> Mollusk {
    let mollusk = Mollusk::new(&PROGRAM, "target/deploy/solana_pinocchio_starter");
    mollusk
}

pub fn get_rent_data() -> Vec<u8> {
    let rent = Rent::default();
    unsafe {
        core::slice::from_raw_parts(&rent as *const Rent as *const u8, Rent::size_of()).to_vec()
    }
}

#[test]
fn test_initialize_mystate() {
    let mollusk = mollusk();

    //system program and system account
    let (system_program, system_account) = program::keyed_account_for_system_program();

    // Create the PDA
    let (mystate_pda, bump) =
        Pubkey::find_program_address(&[MyState::SEED.as_bytes(), &PAYER.to_bytes()], &PROGRAM);

    //Initialize the accounts
    let payer_account = Account::new(1 * LAMPORTS_PER_SOL, 0, &system_program);
    let mystate_account = Account::new(0, 0, &system_program);
    let min_balance = mollusk.sysvars.rent.minimum_balance(Rent::size_of());
    let mut rent_account = Account::new(min_balance, Rent::size_of(), &RENT);
    rent_account.data = get_rent_data();

    //Push the accounts in to the instruction_accounts vec!
    let ix_accounts = vec![
        AccountMeta::new(PAYER, true),
        AccountMeta::new(mystate_pda, false),
        AccountMeta::new_readonly(RENT, false),
        AccountMeta::new_readonly(system_program, false),
    ];

    // Create the instruction data
    let ix_data = Initialize {
        owner: *PAYER.as_array(),
        bump,
    };

    // Ix discriminator = 0
    let mut ser_ix_data = vec![0];

    // Serialize the instruction data
    ser_ix_data.extend_from_slice(unsafe { to_bytes(&ix_data) });

    // Create instruction
    let instruction = Instruction::new_with_bytes(PROGRAM, &ser_ix_data, ix_accounts);

    // Create tx_accounts vec
    let tx_accounts = &vec![
        (PAYER, payer_account.clone()),
        (mystate_pda, mystate_account.clone()),
        (RENT, rent_account.clone()),
        (system_program, system_account.clone()),
    ];

    let init_res =
        mollusk.process_and_validate_instruction(&instruction, tx_accounts, &[Check::success()]);

    assert!(init_res.program_result == ProgramResult::Success);
}
            "#;

            template.replace("{address}", address)
        }
    }
}
