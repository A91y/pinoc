// EXPECT: ACC001-P on `pool` and ACC002-P on `admin`. `Sell::check` makes both
// checks, and this instruction never calls it. `keeper` is the same case as
// `admin`, under a name that is an authority only once the project says so.

pub struct Unwind<'a> {
    pub inner: Sell<'a>,
}

impl<'a> TryFrom<&'a mut [AccountView]> for Unwind<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a mut [AccountView]) -> Result<Self, Self::Error> {
        Ok(Self {
            inner: Sell {
                accounts: SellAccounts::load(accounts)?,
            },
        })
    }
}

impl Unwind<'_> {
    pub fn process(&mut self) -> ProgramResult {
        let a = &self.inner.accounts;
        let config = guards::config(a.config)?;
        guards::expect_admin_address(config, a.admin)?;
        guards::expect_keeper_address(config, a.keeper)?;
        let _reserve = self.inner.pool_reserve()?;
        Ok(())
    }
}
