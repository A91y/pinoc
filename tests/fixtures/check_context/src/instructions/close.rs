// EXPECT: silent. The same checks as `deposit`, written inline.

pub struct CloseAccounts<'a> {
    pub admin: &'a AccountView,
    pub vault: &'a AccountView,
}

impl<'a> TryFrom<&'a [AccountView]> for CloseAccounts<'a> {
    type Error = ProgramError;

    fn try_from(accounts: &'a [AccountView]) -> Result<Self, Self::Error> {
        let [admin, vault, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        if !admin.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        if !vault.owned_by(&crate::ID) {
            return Err(ProgramError::IllegalOwner);
        }
        if vault.data_len() < VaultState::LEN {
            return Err(ProgramError::AccountDataTooSmall);
        }
        Ok(Self { admin, vault })
    }
}

pub struct Close<'a> {
    pub accounts: CloseAccounts<'a>,
}

impl Close<'_> {
    pub fn process(&mut self) -> ProgramResult {
        let accounts = &self.accounts;
        let state = VaultState::from_bytes_unsized(unsafe { accounts.vault.borrow_unchecked() });
        if accounts.admin.address() != &state.authority {
            return Err(ProgramError::IncorrectAuthority);
        }
        Ok(())
    }
}
