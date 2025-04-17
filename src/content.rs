pub mod templates {

    //lib.rs
    pub fn lib_rs() -> &'static str {
        r#"//#![feature(const_mut_refs)]
#![no_std]

#[cfg(not(feature = "no-entrypoint"))]
mod entrypoint;

#[cfg(feature = "std")]
extern crate std;

pub mod errors;
pub mod instructions;
pub mod states;

//After build is successful get the program pubkey and replace it here
pinocchio_pubkey::declare_id!("HhEs5ZBwrR29fQNxELBFdRN7mAvhJNP1R6xgNNL2ZkSD");        "#
    }

    // entrypoint.rs template
    pub fn entrypoint_rs() -> &'static str {
        r#"#![allow(unexpected_cfgs)]
use pinocchio::{
    account_info::AccountInfo, no_allocator, nostd_panic_handler, program_entrypoint,
    program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};
use pinocchio_log::log;

use crate::instructions::{self, VaultInstructions};

program_entrypoint!(process_instruction);
//Do not allocate memory.
no_allocator!();
// Use the no_std panic handler.
#[cfg(target_os = "solana")]
nostd_panic_handler!();

pub fn process_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let (discriminator_variant, instruction_data) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;

    match VaultInstructions::try_from(discriminator_variant)? {
        VaultInstructions::Deposit => {
            log!("Deposit");
            instructions::process_deposit(accounts, instruction_data)?;
        }
        VaultInstructions::Withdraw => {
            log!("Withdraw");
            instructions::process_withdraw(accounts, instruction_data)?;
        }
    }

    Ok(())
}        "#
    }

    // Configuration files
    pub fn readme_md() -> &'static str {
        r#"# chio pinocchio project

A project created with chio CLI.

## Getting Started

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
}
