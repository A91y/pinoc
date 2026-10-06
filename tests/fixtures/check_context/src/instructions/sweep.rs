// The accounts for `processor::process_sweep`, which reads them through a local.

pub struct SweepAccounts<'a> {
    pub vault: &'a AccountView,
    pub destination: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for SweepAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [vault, destination, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        Ok(Self { vault, destination })
    }
}

pub struct Sweep<'a> {
    pub accounts: SweepAccounts<'a>,
}

impl<'a> TryFrom<(&'a [u8], &'a [AccountView])> for Sweep<'a> {
    type Error = ProgramError;

    fn try_from((_data, accounts): (&'a [u8], &'a [AccountView])) -> Result<Self, Self::Error> {
        Ok(Self {
            accounts: SweepAccounts::try_from(accounts)?,
        })
    }
}
