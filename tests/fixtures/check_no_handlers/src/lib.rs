// A crate with state structs and no function taking an accounts slice.

#[repr(C)]
#[derive(ShankAccount)]
pub struct PaddedVault {
    pub bump: u8,
    pub amount: u64,
}

pub fn add(a: u64, b: u64) -> u64 {
    a + b
}
