// Shapes that need the codama 0.13 directives: a variable account tail, an
// array sized by a const, and an instruction with no accounts.

use codama_macros::{CodamaAccount, CodamaInstructions};

pinocchio::address::declare_id!("EfPyA5fx11YAY9XwmrWVddcQe5bBZeNsKA8avqYUH6Qr");

pub const TIER_COUNT: usize = 4;

#[derive(CodamaAccount)]
#[repr(C)]
pub struct Config {
    pub admin: [u8; 32],
    #[codama(type = array(number(u64), 4))]
    pub tiers: [u64; TIER_COUNT],
}

#[derive(CodamaInstructions)]
pub enum Instruction {
    /// Settles every position passed after the fixed accounts.
    #[codama(account(name = "keeper", signer))]
    #[codama(account(name = "vault", writable))]
    #[codama(remaining_accounts(argument("positions"), writable))]
    Settle { amount: u64 },

    /// Takes no accounts at all.
    Ping,
}
