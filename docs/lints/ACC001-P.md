# ACC001-P `missing-owner`

deny · likely · ACC (account validation)

An account read as this program's state without checking that this program owns it. An attacker can pass a look-alike account they created, holding whatever bytes they like.

## What counts

- **Read as state:** the account's data is borrowed (`try_borrow_data`, `borrow_data_unchecked`, ...) or the account is passed to a loader (`Type::load`, `Type::from_bytes`, ...). Sysvar loaders (`Rent::from_account_view`, ...) are not state reads.
- **Identity established:** any one of these clears the finding:
  - the owner was checked (`owner()`, `owned_by`, `is_owned_by`, or an owner mention in a check macro);
  - its address was compared to an expected one (a constant, a stored key, a derived PDA);
  - the handler found it empty (`is_data_empty()`);
  - the handler created it (`CreateAccount { to: account, .. }`).

## Limits

- **Helper functions:** helpers defined in the crate are read, so a check made in `require_owned(vault)` counts.
- **Delegated accounts:** an account handed to a function outside the crate, or to a call that names no single function in the crate, is delegated, and the lint stays quiet about it.
- **Order:** a check counts wherever it appears in the handler or its helpers. pinoc does not prove it runs before the read.
- **Owner target:** an owner check counts whichever owner it names.

## Fix

Check `account.owner() == program_id` (or compare its address to the expected one) before reading its data.

## Flagged

```rust
pub fn read(accounts: &[AccountView]) -> ProgramResult {
    let [vault, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let data = vault.try_borrow_data()?;
    let _state = VaultState::from_bytes(&data)?;
    Ok(())
}
```

## Not flagged

```rust
pub fn read(program_id: &Address, accounts: &[AccountView]) -> ProgramResult {
    let [vault, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if vault.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
    let data = vault.try_borrow_data()?;
    let _state = VaultState::from_bytes(&data)?;
    Ok(())
}
```
