// EXPECT: silent. `check` covers every account `process` goes on to use.

pub const FIXED: usize = 4;

pub struct SellAccounts<'a> {
    pub admin: &'a AccountView,
    pub keeper: &'a AccountView,
    pub config: &'a AccountView,
    pub pool: &'a AccountView,
    pub positions: &'a [AccountView],
}

impl<'a> SellAccounts<'a> {
    pub fn load(accounts: &'a mut [AccountView]) -> Result<Self, ProgramError> {
        if accounts.len() < FIXED {
            return Err(ProgramError::NotEnoughAccountKeys);
        }
        let (head, positions) = accounts.split_at_mut(FIXED);
        let [admin, keeper, config, pool] = head else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        Ok(Self {
            admin,
            keeper,
            config,
            pool,
            positions,
        })
    }
}

pub struct Sell<'a> {
    pub accounts: SellAccounts<'a>,
}

impl<'a> TryFrom<&'a mut [AccountView]> for Sell<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a mut [AccountView]) -> Result<Self, Self::Error> {
        Ok(Self {
            accounts: SellAccounts::load(accounts)?,
        })
    }
}

impl Sell<'_> {
    pub fn process(&mut self) -> ProgramResult {
        self.check()?;
        let _reserve = self.pool_reserve()?;
        Ok(())
    }

    fn check(&self) -> ProgramResult {
        let a = &self.accounts;
        let config = guards::config(a.config)?;
        guards::expect_admin(config, a.admin)?;
        guards::expect_keeper(config, a.keeper)?;
        guards::expect_owned_by(a.pool, &crate::POOL_PROGRAM)?;
        Ok(())
    }

    pub(super) fn pool_reserve(&self) -> Result<u64, ProgramError> {
        let pool = Pool::from_bytes(unsafe { self.accounts.pool.borrow_unchecked() })?;
        Ok(pool.reserve)
    }
}
