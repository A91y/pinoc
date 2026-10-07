// Accounts named by a `match` on the slice.

// EXPECT: ACC001-P on `position`.
pub fn process_batch(accounts: &[AccountView]) -> Result<u64, ProgramError> {
    match accounts {
        [position] => Ok(Position::from_bytes(unsafe { position.borrow_unchecked() })?.amount),
        _ => Err(ProgramError::NotEnoughAccountKeys),
    }
}

// EXPECT: ACC001-P on `oracle`, which leaves the `match` under that name. The
// arm's `config` is not the `config` bound above it, whose owner is checked.
pub fn process_priced(accounts: &[AccountView]) -> Result<u64, ProgramError> {
    let (head, trailing) = accounts.split_at(1);
    let [config] = head else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let (oracle, pool) = match trailing {
        [config, pool] => (Some(config), Some(pool)),
        _ => (None, None),
    };
    let _loaded = guards::config(config)?;
    let oracle = oracle.ok_or(ProgramError::NotEnoughAccountKeys)?;
    let pool = pool.ok_or(ProgramError::NotEnoughAccountKeys)?;
    guards::expect_owned_by(pool, &crate::POOL_PROGRAM)?;
    let _reserve = Pool::from_bytes(unsafe { pool.borrow_unchecked() })?.reserve;
    Ok(Position::from_bytes(unsafe { oracle.borrow_unchecked() })?.amount)
}

// EXPECT: UNTRACKED-ACCOUNTS. The accounts are taken in pairs, which is not
// followed, so the unchecked read below is not reported.
pub fn process_pairs(accounts: &[AccountView]) -> Result<u64, ProgramError> {
    let mut sum = 0u64;
    for pair in accounts.chunks(2) {
        sum += Position::from_bytes(unsafe { pair[0].borrow_unchecked() })?.amount;
    }
    Ok(sum)
}
