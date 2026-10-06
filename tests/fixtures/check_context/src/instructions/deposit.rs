// EXPECT: silent. Every check goes through a `guards` helper.

pub struct DepositAccounts<'a> {
    pub admin: &'a AccountView,
    pub vault: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for DepositAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [admin, vault, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        let loaded = guards::vault(vault)?;
        guards::expect_admin(loaded, admin)?;
        Ok(Self { admin, vault })
    }
}

pub struct Deposit<'a> {
    pub accounts: DepositAccounts<'a>,
    pub amount: u64,
}

impl Deposit<'_> {
    pub fn process(&mut self) -> ProgramResult {
        let vault = guards::vault(self.accounts.vault)?;
        let _total = vault.amount + self.amount;
        Ok(())
    }
}
