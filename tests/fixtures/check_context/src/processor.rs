// EXPECT: ACC001-P on `vault`, read here through a local of the instruction type.

pub fn process_sweep(accounts: &[AccountView], data: &[u8]) -> ProgramResult {
    let ix = Sweep::try_from((data, accounts))?;
    let data = ix.accounts.vault.try_borrow()?;
    let _state = VaultState::from_bytes(&data)?;
    Ok(())
}
