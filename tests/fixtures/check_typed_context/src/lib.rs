// Typed-context style with pinocchio 0.11 method names.

pub struct DepositAccounts<'a> {
    pub owner: &'a AccountView,
    pub vault: &'a AccountView,
}

// EXPECT: UNTRACKED-ACCOUNTS  (the accounts leave `try_from` inside `Self`)
impl<'a> TryFrom<&'a [AccountView]> for DepositAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [owner, vault, _] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        if !owner.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Ok(Self { owner, vault })
    }
}

pub struct Deposit<'a> {
    pub accounts: DepositAccounts<'a>,
}

// EXPECT: silent  (tuple parameter is discovered; the slice is only forwarded)
impl<'a> TryFrom<(&'a [u8], &'a [AccountView])> for Deposit<'a> {
    type Error = ProgramError;

    fn try_from((_data, accounts): (&'a [u8], &'a [AccountView])) -> Result<Self, Self::Error> {
        let accounts = DepositAccounts::try_from(accounts)?;
        Ok(Self { accounts })
    }
}

impl<'a> Deposit<'a> {
    // Not analysed: reads `vault` through `self`, which the fact table does not follow.
    pub fn process(&mut self) -> ProgramResult {
        let _data = unsafe { self.accounts.vault.borrow_unchecked() };
        Ok(())
    }
}

// EXPECT: ZC002-P  (`borrow_unchecked` with no data_len guard; `owned_by` is the owner check)
pub fn read_unchecked(program_id: &Address, accounts: &[AccountView]) -> ProgramResult {
    let [vault, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if !vault.owned_by(program_id) {
        return Err(ProgramError::IllegalOwner);
    }
    let _data = unsafe { vault.borrow_unchecked() };
    Ok(())
}

// EXPECT: ACC001-P  (`next_account_view` binding, mutable unchecked borrow, no owner check)
pub fn write_unowned(accounts: &mut [AccountView]) -> ProgramResult {
    let iter = &mut accounts.iter();
    let vault = next_account_view(iter)?;
    if vault.data_len() < 8 {
        return Err(ProgramError::AccountDataTooSmall);
    }
    let _data = unsafe { vault.borrow_unchecked_mut() };
    Ok(())
}
