// Instruction-arg / defined-type structs.

// EXPECT: ZC003-P  (read zero-copy via the shank derive, but no #[repr(C)])
#[derive(shank::ShankAccount)]
pub struct NoReprState {
    pub count: u64,
    pub flag: u8,
}

// EXPECT: silent  (#[repr(C)] present, and packed 16 == layout 16)
#[repr(C)]
#[derive(shank::ShankAccount)]
pub struct GoodState {
    pub count: u64,
    pub extra: u64,
}

// EXPECT: silent  (#[repr(transparent)] is accepted for zero-copy layout)
#[repr(transparent)]
#[derive(shank::ShankType)]
pub struct Wrapped {
    pub inner: u64,
}
