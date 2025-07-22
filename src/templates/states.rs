
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
