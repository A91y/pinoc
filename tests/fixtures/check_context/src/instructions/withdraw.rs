// EXPECT: ACC001-P on `vault` and ACC002-P on `admin`, both in `process`.
// `try_from` checks nothing; the loader used in `process` checks no owner, and
// the authority helper compares the address without requiring a signature.

pub struct WithdrawAccounts<'a> {
    pub admin: &'a AccountView,
    pub vault: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for WithdrawAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [admin, vault, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        Ok(Self { admin, vault })
    }
}

pub struct Withdraw<'a> {
    pub accounts: WithdrawAccounts<'a>,
}

impl Withdraw<'_> {
    pub fn process(&mut self) -> ProgramResult {
        let vault = guards::vault_unowned(self.accounts.vault)?;
        guards::expect_authority_address(vault, self.accounts.admin)?;
        Ok(())
    }
}
