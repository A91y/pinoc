// The error enum derives `CodamaErrors`, so Codama's own extraction must be kept as is.

use codama::{CodamaAccount, CodamaErrors};
use pinocchio::error::ProgramError;
use thiserror::Error;

pinocchio::address::declare_id!("EfPyA5fx11YAY9XwmrWVddcQe5bBZeNsKA8avqYUH6Qr");

#[derive(CodamaAccount)]
#[repr(C)]
pub struct Counter {
    pub authority: [u8; 32],
    pub count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Error, CodamaErrors)]
pub enum CounterError {
    #[error("Authority does not match the counter")]
    InvalidAuthority,
    #[error("Counter would overflow")]
    CounterOverflow,
}

impl From<CounterError> for ProgramError {
    fn from(e: CounterError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
