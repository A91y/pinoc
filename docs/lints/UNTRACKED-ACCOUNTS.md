# UNTRACKED-ACCOUNTS `untracked-accounts`

warn · definite · coverage

One finding listing the handlers whose accounts were not analysed. Checks and uses in those handlers are not reported, so the run is not a clean result for them.

## What counts

A handler is listed for one of two reasons, and the finding says which:

- it stores its accounts in a struct that no analysed function reads them back from;
- it uses its accounts slice and no account could be bound from it (`chunks`, `iter().find(..)`, an index computed at run time).

At most five handlers are named, with a count of the rest. The finding is at `src:0:0`.

## Fix

- **No code change needed.** Allow `UNTRACKED-ACCOUNTS` once the limit is acknowledged.
- **To get the accounts analysed:** take them with a slice pattern, a constant index, `.get(i)`, or a `for` loop over the slice.
