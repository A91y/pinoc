# NO-HANDLERS `no-handlers`

warn · definite · coverage

No instruction handler was found anywhere, or there is no `src/` directory. The account and CPI lints therefore analysed nothing, and only the struct-layout and discriminator lints ran. This finding replaces `✅ No issues found.`, so an empty result is never mistaken for a clean one.

## What counts

- **Handler:** a free function, `impl` method or trait default method that takes an accounts slice (`&[AccountView]` or `&[AccountInfo]`, also inside a tuple parameter).
- **Location:** reported at `src:0:0`, because it describes the whole crate.

## Fix

- **Not a program:** if the crate is not a program, allow `NO-HANDLERS` (`--allow NO-HANDLERS`, or `allow` under `[check]`).
- **A program:** its handlers take their accounts in a form `pinoc check` does not follow.

`--deny NO-HANDLERS` (or `--deny all`) makes CI fail on a run that analysed nothing.
