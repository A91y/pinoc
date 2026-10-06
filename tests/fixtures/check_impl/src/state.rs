// Zero-copy account/state structs. Each struct is labeled with the finding it
// should (or should not) produce under `pinoc check`.

// EXPECT: ZC001-P  (padded: owner 32 + amount 8 + bump 1 = 41 packed, but
// #[repr(C)] rounds to 48; 7 bytes of padding)
#[repr(C)]
#[derive(shank::ShankAccount)]
pub struct PaddedVault {
    pub owner: [u8; 32],
    pub amount: u64,
    pub bump: u8,
}

// EXPECT: silent  (naturally packed: 32 + 8 = 40, no padding)
#[repr(C)]
#[derive(shank::ShankType)]
pub struct PackedVault {
    pub owner: [u8; 32],
    pub amount: u64,
}

// EXPECT: silent  (same fields as PaddedVault, but explicit _padding makes the
// #[repr(C)] size match the packed borsh size, so it round-trips)
#[repr(C)]
#[derive(shank::ShankAccount)]
pub struct FixedVault {
    pub owner: [u8; 32],
    pub amount: u64,
    pub bump: u8,
    pub _padding: [u8; 7],
}

// EXPECT: silent while the allow is present; remove the line to see ZC001-P fire
// pinoc:allow(ZC001-P) padding is intentional here
#[repr(C)]
#[derive(shank::ShankAccount)]
pub struct SuppressedVault {
    pub owner: [u8; 32],
    pub amount: u64,
    pub bump: u8,
}
