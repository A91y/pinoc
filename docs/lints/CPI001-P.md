# CPI001-P `arbitrary-cpi`

warn · likely · CPI (cross-program invocation)

`invoke` or `invoke_signed` whose program id comes from an account the caller supplied, and whose key is never compared to an expected id. An attacker redirects the call to a program they wrote.

## What counts

- **Fires when:** the `program_id` of the instruction passed to `invoke`/`invoke_signed` resolves to an account binding.
- **Cleared:** that account's key is compared to an expected id (`==`/`!=`, `.eq()`/`.ne()`, or in a check macro), in the handler or in a crate-local helper it is passed to.
- **Ignored:** typed-builder CPIs (`Transfer { .. }.invoke()`, `CreateAccount { .. }.invoke_signed(..)`) and constant or parameter program ids, since the builder or the constant fixes the program.

## Limits

- **Intermediate locals:** a `program_id` that reaches the instruction through intermediate locals (`let pid = account.address(); .. program_id: pid`) is not followed.
- **Delegated accounts:** a delegated program account is left alone.
- **Default severity:** `warn` until the false-positive corpus supports `deny`. Promote it with `--deny CPI001-P`.

## Fix

Compare the program account's key to the expected program id before invoking.

## Flagged

```rust
pub fn forward(accounts: &[AccountView], data: &[u8]) -> ProgramResult {
    let [target, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let ix = InstructionView {
        program_id: target.address(),
        accounts: &[],
        data,
    };
    invoke(&ix, &[])
}
```

## Not flagged

```rust
pub fn forward(accounts: &[AccountView], data: &[u8]) -> ProgramResult {
    let [target, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    if target.address() != &EXPECTED_PROGRAM {
        return Err(ProgramError::IncorrectProgramId);
    }
    let ix = InstructionView {
        program_id: target.address(),
        accounts: &[],
        data,
    };
    invoke(&ix, &[])
}
```
