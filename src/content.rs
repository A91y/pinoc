pub mod templates {

    //lib.rs

    pub fn lib_rs(address: &str) -> String {
        format!(
            r#"//#![feature(const_mut_refs)]
#![no_std]

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

use crate::instruction::{self, MyProgramInstruction};
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

    match MyProgramInstruction::try_from(ix_disc)? {
        MyProgramInstruction::Initialize => {
            msg!("Initialize");
            instruction::initilaize(accounts, instruction_data)
        }
    }
}       "#
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

#[derive(Clone, PartialEq)]
pub enum MyProgramError {
    // overflow error
    WriteOverflow,
    // invalid instruction data
    InvalidInstructionData,
    // pda mismatch
    PdaMismatch,
    // Invalid Owner
    InvalidOwner,
    // Not a system account
    InvalidAccount,
    //Incorect Vault
    IncorrectVaultAcc,
}

impl From<MyProgramError> for ProgramError {
    fn from(e: MyProgramError) -> Self {
        Self::Custom(e as u32)
    }
}
            "#
    }

    pub mod instructions {
        pub fn deposit_rs() -> &'static str {
            r#"use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey, ProgramResult};

use pinocchio_system::instructions::Transfer;

pub const LAMPORTS: u64 = 1_000_000_000;

use crate::states::{load_ix_data, DataLen};

#[repr(C)]
pub struct DepositData {
    pub amount: u64,
    pub bump: u8,
}

impl DataLen for DepositData {
    const LEN: usize = core::mem::size_of::<DepositData>();
}

pub fn process_deposit(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let [deposit_account, vault_account, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !deposit_account.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // check the CU after implementing unsafe {} block here
    let ix_data = load_ix_data::<DepositData>(data)?;

    let vault_pda = pubkey::create_program_address(
        &["vault".as_bytes(), deposit_account.key(), &[ix_data.bump]],
        &crate::ID,
    )?;

    if vault_account.key() != &vault_pda {
        return Err(ProgramError::InvalidAccountData);
    }

    Transfer {
        from: deposit_account,
        to: vault_account,
        lamports: ix_data.amount * LAMPORTS,
    }
    .invoke()?;

    Ok(())
}"#
        }

        pub fn instructions_mod_rs() -> &'static str {
            r#"use pinocchio::program_error::ProgramError;

pub mod deposit;
pub mod withdraw;

pub use deposit::*;
pub use withdraw::*;

#[repr(u8)]
pub enum VaultInstructions {
    Deposit,
    Withdraw,
}

impl TryFrom<&u8> for VaultInstructions {
    type Error = ProgramError;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        match *value {
            0 => Ok(VaultInstructions::Deposit),
            1 => Ok(VaultInstructions::Withdraw),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}"#
        }

        pub fn withdraw_rs() -> &'static str {
            r#"use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::{self},
    ProgramResult,
};
use pinocchio_system::instructions::Transfer;

use crate::errors::MyProgramError;

pub fn process_withdraw(accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let [withtdraw_account, vault_account, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !withtdraw_account.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if !vault_account.data_is_empty() && vault_account.lamports() > 0 {
        return Err(MyProgramError::InvalidAccount.into());
    }

    let bump = data[0];

    let seed = ["vault".as_bytes(), withtdraw_account.key(), &[bump]];
    let vault_pda = pubkey::create_program_address(&seed, &crate::ID)?;

    if vault_pda != *vault_account.key() {
        return Err(MyProgramError::IncorrectVaultAcc.into());
    };

    let pda_byte_bump = [bump];
    let signer_seed = [
        Seed::from("vault".as_bytes()),
        Seed::from(withtdraw_account.key()),
        Seed::from(&pda_byte_bump),
    ];

    let signer = [Signer::from(&signer_seed)];

    Transfer {
        from: vault_account,
        to: withtdraw_account,
        lamports: vault_account.lamports(),
    }
    .invoke_signed(&signer)?;

    Ok(())
}"#
        }
    }

    pub mod states {
        pub fn states_mod_rs() -> &'static str {
            r#"pub mod utils;

pub use utils::*;

            "#
        }

        pub fn utils_rs() -> &'static str {
            r#"use pinocchio::program_error::ProgramError;

use crate::errors::MyProgramError;

pub trait DataLen {
    const LEN: usize;
}

pub trait Initialized {
    fn is_initialized(&self) -> bool;
}

#[inline(always)]
pub fn load_acc<T: DataLen + Initialized>(bytes: &[u8]) -> Result<&T, ProgramError> {
    load_acc_unchecked::<T>(bytes).and_then(|acc| {
        if acc.is_initialized() {
            Ok(acc)
        } else {
            Err(ProgramError::UninitializedAccount)
        }
    })
}

#[inline(always)]
pub fn load_acc_unchecked<T: DataLen>(bytes: &[u8]) -> Result<&T, ProgramError> {
    if bytes.len() != T::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(unsafe { &*(bytes.as_ptr() as *const T) })
}

#[inline(always)]
pub fn load_acc_mut<T: DataLen + Initialized>(bytes: &mut [u8]) -> Result<&mut T, ProgramError> {
    load_acc_mut_unchecked::<T>(bytes).and_then(|acc| {
        if acc.is_initialized() {
            Ok(acc)
        } else {
            Err(ProgramError::UninitializedAccount)
        }
    })
}

#[inline(always)]
pub fn load_acc_mut_unchecked<T: DataLen>(bytes: &mut [u8]) -> Result<&mut T, ProgramError> {
    if bytes.len() != T::LEN {
        return Err(ProgramError::InvalidAccountData);
    }
    Ok(unsafe { &mut *(bytes.as_mut_ptr() as *mut T) })
}

#[inline(always)]
pub fn load_ix_data<T: DataLen>(bytes: &[u8]) -> Result<&T, ProgramError> {
    if bytes.len() != T::LEN {
        return Err(MyProgramError::InvalidInstructionData.into());
    }
    Ok(unsafe { &*(bytes.as_ptr() as *const T) })
}

pub fn to_bytes<T: DataLen>(data: &T) -> &[u8] {
    unsafe { core::slice::from_raw_parts(data as *const T as *const u8, T::LEN) }
}

pub fn to_mut_bytes<T: DataLen>(data: &mut T) -> &mut [u8] {
    unsafe { core::slice::from_raw_parts_mut(data as *mut T as *mut u8, T::LEN) }
}"#
        }
    }

    pub mod unit_tests {
        pub fn unit_test_rs(address: &String) -> String {
            let template = r#"
use mollusk_svm::result::{Check, ProgramResult};
use mollusk_svm::{program, Mollusk};
use pinocchio_vault::instructions::DepositData;
use pinocchio_vault::states::to_bytes;
use pinocchio_vault::ID;
use solana_sdk::account::Account;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::native_token::LAMPORTS_PER_SOL;
use solana_sdk::pubkey;
use solana_sdk::pubkey::Pubkey;

pub const PROGRAM: Pubkey = Pubkey::new_from_array(ID);

pub const RENT: Pubkey = pubkey!("SysvarRent111111111111111111111111111111111");

pub const PAYER: Pubkey = pubkey!("{address}");

pub fn mollusk() -> Mollusk {
    Mollusk::new(&PROGRAM, "target/deploy/pinocchio_vault")
}

#[test]
fn test_deposit() {
    let mollusk = mollusk();

    let (system_prgram, system_account) = program::keyed_account_for_system_program();

    let (vault_pda, bump) =
        Pubkey::find_program_address(&["vault".as_bytes(), &PAYER.to_bytes()], &PROGRAM);

    let payer_acc = Account::new(10 * LAMPORTS_PER_SOL, 0, &system_prgram);
    let vault_acc = Account::new(0, 0, &system_prgram);

    let ix_account = vec![
        AccountMeta::new(PAYER, true),
        AccountMeta::new(vault_pda, false),
        AccountMeta::new(system_prgram, false),
    ];

    let ix_data = DepositData { amount: 1, bump };

    let mut ser_ix_data = vec![0];

    ser_ix_data.extend_from_slice(to_bytes(&ix_data));

    let instruction = Instruction::new_with_bytes(PROGRAM, &ser_ix_data, ix_account);

    let tx_accounts = &vec![
        (PAYER, payer_acc.clone()),
        (vault_pda, vault_acc.clone()),
        (system_prgram, system_account.clone()),
    ];

    let init_res =
        mollusk.process_and_validate_instruction(&instruction, tx_accounts, &[Check::success()]);

    assert!(init_res.program_result == ProgramResult::Success);
}

#[test]
fn test_withdraw() {
    let mollusk = mollusk();

    let (system_prgram, system_account) = program::keyed_account_for_system_program();

    let (vault_pda, bump) =
        Pubkey::find_program_address(&["vault".as_bytes(), &PAYER.to_bytes()], &PROGRAM);

    let payer_acc = Account::new(9, 0, &system_prgram);
    let vault_acc = Account::new(1, 0, &system_prgram);

    let ix_account = vec![
        AccountMeta::new(PAYER, true),
        AccountMeta::new(vault_pda, false),
        AccountMeta::new(system_prgram, false),
    ];

    let mut ix_data = vec![1];

    ix_data.push(bump);

    let instruction = Instruction::new_with_bytes(PROGRAM, &ix_data, ix_account);

    let tx_accounts = &vec![
        (PAYER, payer_acc.clone()),
        (vault_pda, vault_acc.clone()),
        (system_prgram, system_account.clone()),
    ];

    let update_res =
        mollusk.process_and_validate_instruction(&instruction, tx_accounts, &[Check::success()]);

    assert!(update_res.program_result == ProgramResult::Success);
}
            "#;

            template.replace("{address}", address)
        }
    }
}
