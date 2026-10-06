// Enums whose discriminants are not their variant positions, declared for both
// the Codama and the shank extractor.

use codama_macros::{CodamaAccount, CodamaInstructions, CodamaType};
use shank::{ShankAccount, ShankInstruction, ShankType};

pinocchio::address::declare_id!("EfPyA5fx11YAY9XwmrWVddcQe5bBZeNsKA8avqYUH6Qr");

#[derive(CodamaType, ShankType)]
#[repr(u8)]
pub enum Status {
    Open = 1,
    Active = 2,
    Closed = 5,
}

#[derive(CodamaType, ShankType)]
#[repr(u8)]
pub enum Plain {
    A,
    B,
}

#[derive(CodamaAccount, ShankAccount)]
#[repr(C)]
pub struct Vault {
    pub status: Status,
    pub plain: Plain,
}

#[derive(CodamaInstructions, ShankInstruction)]
pub enum Instruction {
    #[codama(account(name = "vault", writable))]
    #[account(0, writable, name = "vault")]
    SetStatus { status: Status },
}
