// An account at a program-derived address, and constants exported by marker.

use codama_macros::{CodamaAccount, CodamaInstructions};

pinocchio::address::declare_id!("EfPyA5fx11YAY9XwmrWVddcQe5bBZeNsKA8avqYUH6Qr");

/// Open positions a trader may hold.
// pinoc:constant
pub const MAX_OPEN_POSITIONS: u8 = 8;

// pinoc:constant(u8)
pub const MAX_ATAS: usize = 12;

const MILLION: u64 = 1_000_000;

// pinoc:constant
/// Ceiling on the reserve floor.
pub const MAX_RESERVE_FLOOR: u64 = 100_000_000 * MILLION;

/// Not exported: it carries no marker.
pub const INTERNAL_LIMIT: u64 = 7;

#[derive(CodamaAccount)]
#[codama(seed(type = string(utf8), value = "vault"))]
#[repr(C)]
pub struct Vault {
    pub owner: [u8; 32],
    pub amount: u64,
}

#[derive(CodamaInstructions)]
pub enum Instruction {
    #[codama(account(name = "vault", writable))]
    Deposit { amount: u64 },
}
