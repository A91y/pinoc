// Accounts taken out of a slice one at a time.

// EXPECT: silent. Each account is pinned to a registered address before it is read.
pub fn expect_closed(registered: &[Address], tail: &[AccountView]) -> ProgramResult {
    let mut at = 0usize;
    for expected in registered {
        let account = tail.get(at).ok_or(ProgramError::NotEnoughAccountKeys)?;
        at += 1;
        if account.address() != expected {
            return Err(ProgramError::InvalidAccountData);
        }
        let position = Position::from_bytes(unsafe { account.borrow_unchecked() })?;
        if position.amount != 0 {
            return Err(ProgramError::InvalidAccountData);
        }
    }
    Ok(())
}

// EXPECT: ACC001-P on `account`. Nothing says whose accounts these are.
pub fn total(tail: &[AccountView]) -> Result<u64, ProgramError> {
    let mut sum = 0u64;
    for account in tail.iter() {
        let position = Position::from_bytes(unsafe { account.borrow_unchecked() })?;
        sum += position.amount;
    }
    Ok(sum)
}

// EXPECT: ACC001-P on `last`.
pub fn newest(tail: &[AccountView]) -> Result<u64, ProgramError> {
    let last = tail.last().ok_or(ProgramError::NotEnoughAccountKeys)?;
    Ok(Position::from_bytes(unsafe { last.borrow_unchecked() })?.amount)
}
