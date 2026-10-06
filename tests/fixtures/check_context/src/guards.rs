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

/// Signer and address, under one name.
pub fn expect_admin(vault: &VaultState, account: &AccountView) -> Result<(), ProgramError> {
    expect_signer(account)?;
    if account.address() != &vault.authority {
        return Err(ProgramError::IncorrectAuthority);
    }
    Ok(())
}

/// Address only: the caller still has to require a signature.
pub fn expect_authority_address(
    vault: &VaultState,
    account: &AccountView,
) -> Result<(), ProgramError> {
    if account.address() != &vault.authority {
        return Err(ProgramError::IncorrectAuthority);
    }
    Ok(())
}

/// Owner-checked loader: the only checked route to the state.
pub fn vault(account: &AccountView) -> Result<&VaultState, ProgramError> {
    expect_owned_by(account, &crate::ID)?;
    VaultState::from_bytes(unsafe { account.borrow_unchecked() })
}

/// Loads without checking who owns the account.
pub fn vault_unowned(account: &AccountView) -> Result<&VaultState, ProgramError> {
    VaultState::from_bytes(unsafe { account.borrow_unchecked() })
}

/// Returns an account out of the slice.
pub fn next(accounts: &[AccountView], index: usize) -> Result<&AccountView, ProgramError> {
    accounts.get(index).ok_or(ProgramError::NotEnoughAccountKeys)
}
