// Instruction handlers, exercised by ACC001-P (missing-owner). Each is labeled
// with the finding it should (or should not) produce.

// EXPECT: ACC001-P  (borrows the account's data, never checks its owner)
impl ReadUnchecked {
    pub fn process(accounts: &[AccountView]) -> ProgramResult {
        let [vault, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        let data = vault.try_borrow_data()?;
        let _state = VaultState::from_bytes(&data)?;
        Ok(())
    }
}

// EXPECT: silent  (owner verified before the read)
impl<'a> Handler<'a> for ReadChecked {
    fn process(program_id: &Address, accounts: &[AccountView]) -> ProgramResult {
        let [vault, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        if vault.owner() != program_id {
            return Err(ProgramError::IllegalOwner);
        }
        let _data = vault.try_borrow_data()?;
        Ok(())
    }
}

// EXPECT: silent  (owner check factored into a helper; the account is delegated,
// so the lint stays quiet instead of guessing)
pub trait ReadDelegated {
    fn process(accounts: &[AccountView]) -> ProgramResult {
        let [vault, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        require_owned(vault)?;
        let _data = vault.try_borrow_data()?;
        Ok(())
    }
}

// EXPECT: silent  (reads a sysvar, not program state)
impl ReadSysvar {
    pub fn process(accounts: &[AccountView]) -> ProgramResult {
        let [rent_acc, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        let _rent = Rent::from_account_view(rent_acc)?;
        Ok(())
    }
}

// EXPECT: ZC002-P  (owner checked, but the unchecked borrow has no data_len
// guard; a shorter-than-expected account is read past its end)
impl<'a> Handler<'a> for CastUnchecked {
    fn process(program_id: &Address, accounts: &[AccountView]) -> ProgramResult {
        let [vault, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        if vault.owner() != program_id {
            return Err(ProgramError::IllegalOwner);
        }
        let data = unsafe { vault.borrow_data_unchecked() };
        let _state = VaultState::from_bytes(data)?;
        Ok(())
    }
}

// EXPECT: ACC002-P  (identity checked against the stored authority, but the
// account is never verified as a signer, so anyone can pass its address)
pub trait WithdrawNoSigner {
    fn process(program_id: &Address, accounts: &[AccountView]) -> ProgramResult {
        let [vault, authority, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        if vault.owner() != program_id {
            return Err(ProgramError::IllegalOwner);
        }
        let state = VaultState::load(vault)?;
        if authority.address() != &state.authority {
            return Err(ProgramError::IncorrectAuthority);
        }
        Ok(())
    }
}

// EXPECT: silent  (the authority is required to sign)
impl WithdrawChecked {
    pub fn process(program_id: &Address, accounts: &[AccountView]) -> ProgramResult {
        let [vault, authority, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        if vault.owner() != program_id {
            return Err(ProgramError::IllegalOwner);
        }
        let state = VaultState::load(vault)?;
        if authority.address() != &state.authority {
            return Err(ProgramError::IncorrectAuthority);
        }
        if !authority.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Ok(())
    }
}

// EXPECT: ACC003-P  (config read as trusted state, but its key is never checked
// against the expected address, so an attacker can pass a look-alike config).
// Heuristic: hidden at the default `likely` threshold; see with `--deny ACC003-P`
// or `confidence_threshold = "heuristic"`.
impl<'a> Handler<'a> for ClaimRewards {
    fn process(program_id: &Address, accounts: &[AccountView]) -> ProgramResult {
        let [config, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        if config.owner() != program_id {
            return Err(ProgramError::IllegalOwner);
        }
        let _cfg = Config::load(config)?;
        Ok(())
    }
}

// EXPECT: silent  (config identity is verified before use)
pub trait ClaimRewardsChecked {
    fn process(program_id: &Address, accounts: &[AccountView]) -> ProgramResult {
        let [config, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        if config.owner() != program_id {
            return Err(ProgramError::IllegalOwner);
        }
        if config.address() != &CONFIG_PDA {
            return Err(ProgramError::InvalidAccount);
        }
        let _cfg = Config::load(config)?;
        Ok(())
    }
}

// EXPECT: silent  (data_len checked before the unchecked borrow)
impl CastLengthChecked {
    pub fn process(program_id: &Address, accounts: &[AccountView]) -> ProgramResult {
        let [vault, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        if vault.owner() != program_id {
            return Err(ProgramError::IllegalOwner);
        }
        if vault.data_len() < core::mem::size_of::<VaultState>() {
            return Err(ProgramError::AccountDataTooSmall);
        }
        let data = unsafe { vault.borrow_data_unchecked() };
        let _state = VaultState::from_bytes(data)?;
        Ok(())
    }
}

// EXPECT: CPI001-P  (invokes a program taken straight from a caller-supplied
// account whose key is never compared to an expected id)
impl<'a> Handler<'a> for CpiArbitrary {
    fn process(accounts: &[AccountView]) -> ProgramResult {
        let [target, payer, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        let ix = InstructionView {
            program_id: target.address(),
            accounts: &[],
            data: &[],
        };
        invoke(&ix, &[target, payer])?;
        Ok(())
    }
}

// EXPECT: silent  (the program account's key is compared before the invoke)
pub trait CpiChecked {
    fn process(accounts: &[AccountView]) -> ProgramResult {
        let [target, payer, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        if target.address() != &pinocchio_token::ID {
            return Err(ProgramError::IncorrectProgramId);
        }
        let ix = InstructionView {
            program_id: target.address(),
            accounts: &[],
            data: &[],
        };
        invoke(&ix, &[target, payer])?;
        Ok(())
    }
}

// EXPECT: silent  (program is a hardcoded constant, not a caller-supplied account)
impl CpiConst {
    pub fn process(accounts: &[AccountView]) -> ProgramResult {
        let [payer, ..] = accounts else {
            return Err(ProgramError::NotEnoughAccountKeys);
        };
        let ix = InstructionView {
            program_id: &pinocchio_system::ID,
            accounts: &[],
            data: &[],
        };
        invoke(&ix, &[payer])?;
        Ok(())
    }
}
