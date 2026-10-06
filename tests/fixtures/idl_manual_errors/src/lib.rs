// Codama derives for accounts, but the error enum only has a hand-written
// `impl From<_> for ProgramError` (no `CodamaErrors`, no `thiserror::Error`).

use codama::CodamaAccount;
use pinocchio::error::ProgramError;

pinocchio::address::declare_id!("EfPyA5fx11YAY9XwmrWVddcQe5bBZeNsKA8avqYUH6Qr");

#[derive(CodamaAccount)]
#[repr(C)]
pub struct Counter {
    pub authority: [u8; 32],
    pub count: u64,
}

pub enum CounterError {
    InvalidAuthority,
    CounterOverflow = 6,
    NotInitialized,
}

impl From<CounterError> for ProgramError {
    fn from(e: CounterError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
