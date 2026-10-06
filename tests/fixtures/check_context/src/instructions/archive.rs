// EXPECT: UNTRACKED-ACCOUNTS. The accounts are stored, and nothing in the crate
// reads them back through `ArchiveAccounts`.

pub struct ArchiveAccounts<'a> {
    pub vault: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for ArchiveAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [vault, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        Ok(Self { vault })
    }
}
