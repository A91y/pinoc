
pub fn initialize() -> &'static str {
    r#"use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::rent::Rent,
    Address, AccountView, ProgramResult,
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
#[derive(Clone, Copy, Debug, PartialEq, shank::ShankType)]
pub struct Initialize {
    pub owner: Address,
    pub bump: u8,
}

impl DataLen for Initialize {
    const LEN: usize = core::mem::size_of::<Initialize>();
}

pub fn initialize(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
    let [payer_acc, state_acc, sysvar_rent_acc, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !payer_acc.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if !state_acc.is_data_empty() {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    let rent = Rent::from_account_view(sysvar_rent_acc)?;

    let ix_data = unsafe { load_ix_data::<Initialize>(data)? };

    if ix_data.owner.ne(payer_acc.address()) {
        return Err(MyProgramError::InvalidOwner.into());
    }

    let pda_bump_bytes = [ix_data.bump];

    MyState::validate_pda(ix_data.bump, state_acc.address(), &ix_data.owner)?;

    let signer_seeds = [
        Seed::from(MyState::SEED.as_bytes()),
        Seed::from(ix_data.owner.as_array()),
        Seed::from(&pda_bump_bytes[..]),
    ];
    let signers = [Signer::from(&signer_seeds[..])];

    CreateAccount {
        from: payer_acc,
        to: state_acc,
        space: MyState::LEN as u64,
        owner: &crate::ID,
        lamports: rent.try_minimum_balance(MyState::LEN)?,
    }
    .invoke_signed(&signers)?;

    MyState::initialize(state_acc, ix_data)?;

    Ok(())
}"#
}

pub fn instructions_mod_rs() -> &'static str {
    r#"use pinocchio::error::ProgramError;

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
}

/// IDL-only mirror of `ProgramInstruction`, read by `shank idl` (via `pinoc idl`) to describe
/// each instruction's accounts and args. Never constructed at runtime.
#[derive(shank::ShankInstruction)]
#[allow(dead_code)]
enum ProgramInstructions {
    #[account(0, writable, signer, name = "payer", desc = "Account paying for the new state account")]
    #[account(1, writable, name = "state", desc = "State PDA to be created")]
    #[account(2, name = "rent", desc = "Rent sysvar")]
    #[account(3, name = "system_program", desc = "System program")]
    InitializeState(Initialize),
}"#
}
