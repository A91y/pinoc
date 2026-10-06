// EXPECT: ZC002-P on `vault` in `process`. The owner is checked, but the bytes
// are handed to a cast that does not bound them.

pub struct SetFeeAccounts<'a> {
    pub vault: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for SetFeeAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let vault = guards::next(accounts, 0)?;
        guards::expect_owned_by(vault, &crate::ID)?;
        Ok(Self { vault })
    }
}

impl SetFeeAccounts<'_> {
    pub fn process(&self) -> ProgramResult {
        let _state = VaultState::from_bytes_unsized(unsafe { self.vault.borrow_unchecked() });
        Ok(())
    }
}
