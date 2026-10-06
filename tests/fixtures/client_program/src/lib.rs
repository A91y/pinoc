// A shank-annotated program with two instructions, one account, and one error enum.

use pinocchio::error::ProgramError;
use shank::{ShankAccount, ShankInstruction, ShankType};

pinocchio::address::declare_id!("EfPyA5fx11YAY9XwmrWVddcQe5bBZeNsKA8avqYUH6Qr");

#[repr(C)]
#[derive(ShankAccount)]
pub struct Vault {
    pub owner: [u8; 32],
    pub amount: u64,
}

#[repr(C)]
#[derive(ShankType)]
pub struct DepositArgs {
    pub amount: u64,
}

#[derive(ShankInstruction)]
pub enum VaultInstruction {
    #[account(0, writable, signer, name = "owner", desc = "Vault owner")]
    #[account(1, writable, name = "vault", desc = "Vault account")]
    Deposit(DepositArgs),
    #[account(0, writable, signer, name = "owner", desc = "Vault owner")]
    #[account(1, writable, name = "vault", desc = "Vault account")]
    CloseVault,
}

#[derive(Debug, thiserror::Error)]
pub enum VaultError {
    #[error("Vault owner mismatch")]
    OwnerMismatch,
}

impl From<VaultError> for ProgramError {
    fn from(e: VaultError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
