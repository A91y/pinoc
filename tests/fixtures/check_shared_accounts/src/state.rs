pub struct Config {
    pub authority: Address,
    pub keeper: Address,
}

pub struct Pool {
    pub reserve: u64,
}

pub struct Position {
    pub amount: u64,
}

impl Config {
    pub const LEN: usize = 64;

    /// Bounds the slice, then casts.
    pub fn from_bytes(data: &[u8]) -> Result<&Self, ProgramError> {
        if data.len() < Self::LEN {
            return Err(ProgramError::AccountDataTooSmall);
        }
        Ok(unsafe { &*(data.as_ptr() as *const Self) })
    }
}

impl Pool {
    pub const LEN: usize = 8;

    /// Bounds the slice, then casts.
    pub fn from_bytes(data: &[u8]) -> Result<&Self, ProgramError> {
        if data.len() < Self::LEN {
            return Err(ProgramError::AccountDataTooSmall);
        }
        Ok(unsafe { &*(data.as_ptr() as *const Self) })
    }
}

impl Position {
    pub const LEN: usize = 8;

    /// Bounds the slice, then casts.
    pub fn from_bytes(data: &[u8]) -> Result<&Self, ProgramError> {
        if data.len() < Self::LEN {
            return Err(ProgramError::AccountDataTooSmall);
        }
        Ok(unsafe { &*(data.as_ptr() as *const Self) })
    }
}
