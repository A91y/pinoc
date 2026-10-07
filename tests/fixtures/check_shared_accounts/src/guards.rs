pub fn expect_signer(account: &AccountView) -> Result<(), ProgramError> {
    if !account.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    Ok(())
}

pub fn expect_owned_by(account: &AccountView, owner: &Address) -> Result<(), ProgramError> {
    if !account.owned_by(owner) {
        return Err(ProgramError::IllegalOwner);
    }
    Ok(())
}

/// Owner-checked loader.
pub fn config(account: &AccountView) -> Result<&Config, ProgramError> {
    expect_owned_by(account, &crate::ID)?;
    Config::from_bytes(unsafe { account.borrow_unchecked() })
}

/// Signer and address.
pub fn expect_admin(config: &Config, account: &AccountView) -> Result<(), ProgramError> {
    expect_signer(account)?;
    expect_admin_address(config, account)
}

/// Address only: the caller still has to require a signature.
pub fn expect_admin_address(config: &Config, account: &AccountView) -> Result<(), ProgramError> {
    if account.address() != &config.authority {
        return Err(ProgramError::IncorrectAuthority);
    }
    Ok(())
}

/// Signer and address.
pub fn expect_keeper(config: &Config, account: &AccountView) -> Result<(), ProgramError> {
    expect_signer(account)?;
    expect_keeper_address(config, account)
}

/// Address only: the caller still has to require a signature.
pub fn expect_keeper_address(config: &Config, account: &AccountView) -> Result<(), ProgramError> {
    if account.address() != &config.keeper {
        return Err(ProgramError::IncorrectAuthority);
    }
    Ok(())
}
