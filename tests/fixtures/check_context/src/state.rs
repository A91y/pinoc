pub struct VaultState {
    pub authority: Address,
    pub amount: u64,
}

impl VaultState {
    pub const LEN: usize = 40;

    /// Bounds the slice, then casts.
    pub fn from_bytes(data: &[u8]) -> Result<&Self, ProgramError> {
        Self::check_bytes(data)?;
        Ok(unsafe { &*(data.as_ptr() as *const Self) })
    }

    fn check_bytes(data: &[u8]) -> Result<(), ProgramError> {
        if data.len() < Self::LEN {
            return Err(ProgramError::AccountDataTooSmall);
        }
        Ok(())
    }

    /// Casts without looking at the length.
    pub fn from_bytes_unsized(data: &[u8]) -> &Self {
        unsafe { &*(data.as_ptr() as *const Self) }
    }
}
