# ACC003-P `account-confusion`

warn · heuristic · ACC (account validation)

An account named like a global singleton (`config`, `settings`, `*_config`, `*_settings`) read as trusted state without its key ever being compared to an expected address. An attacker can substitute a look-alike account with contents they chose.

**Hidden by default:** at the default `likely` threshold this lint is hidden. Show it with `confidence_threshold = "heuristic"` or `--deny ACC003-P`.

## Why heuristic

- **Singleton or per-instance:** the engine cannot tell a true singleton (one account, whose key must equal a fixed or derived address) from a per-instance account (one per user, pool or position, whose key legitimately varies).
- **Name only:** a per-instance account is validated by deriving its PDA and comparing it. To a name heuristic, `pool_config` looks like a global `config`, so it would overfire.
- **What would promote it:** telling the two apart from the PDA seeds (constant seeds mean a singleton), which needs following seed values.

## What counts

- **Fires when:** the account's name matches, its data is read, and its key is never compared (`==`/`!=`, `.eq()`/`.ne()`, or in a check macro).
- **Cleared:** a comparison against a derived PDA clears it.
- **Delegated accounts:** a delegated account is left alone.

## Fix

Compare the account's key to the expected address, or to its derived PDA, before reading it.

## Flagged

```rust
pub fn update(program_id: &Address, accounts: &[AccountView]) -> ProgramResult {
    let [config, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if config.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
    let _state = Config::load(config)?;
    Ok(())
}
```

## Not flagged

```rust
pub fn update(program_id: &Address, accounts: &[AccountView]) -> ProgramResult {
    let [config, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if config.address() != &CONFIG_ADDRESS {
        return Err(ProgramError::InvalidAccountData);
    }
    let _state = Config::load(config)?;
    Ok(())
}
```
