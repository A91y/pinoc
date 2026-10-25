# ACC002-P `missing-signer`

warn · likely · ACC (account validation)

An account whose key is compared against a stored authority but which is never required to sign. Anyone can act as that authority by passing its public address: identity is not authorization.

## What counts

- **Authority position:** the account's key (`key()`/`address()`) is compared, with `==`/`!=` or `.eq()`/`.ne()`, against a field or variable named like an authority: `authority`, `admin`, `auth`, a name ending in `_authority` or `_auth`, or a name listed in `[check] authority_names` in `Pinoc.toml`.
- **Signer check:** `is_signer()` on the account, in the handler or in a crate-local helper it is passed to, or inside a check macro, clears the finding.

## Limits

- **Name match:** an authority stored under another name (`controller`, `gov`) is missed unless it is added to `authority_names`. This is deliberate. Treating every comparison against a loaded state field as an authority check would also match a stored `mint` or `bump`, and overfire.
- **Downstream enforcement:** a signer that another program enforces downstream (the token program requires the transfer authority to sign) is not visible, which is why the default is `warn`. Promote it with `--deny ACC002-P`.
- **Delegated accounts:** a delegated account is left alone.

## Fix

Require `authority.is_signer()` before acting on its authority.

## Flagged

```rust
pub fn withdraw(program_id: &Address, accounts: &[AccountView]) -> ProgramResult {
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
```

## Not flagged

```rust
pub fn withdraw(program_id: &Address, accounts: &[AccountView]) -> ProgramResult {
    let [vault, authority, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if vault.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
    if !authority.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    let state = VaultState::load(vault)?;
    if authority.address() != &state.authority {
        return Err(ProgramError::IncorrectAuthority);
    }
    Ok(())
}
```
