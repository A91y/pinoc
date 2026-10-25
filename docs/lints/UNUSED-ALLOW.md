# UNUSED-ALLOW `unused-allow`

warn · definite · meta

A `// pinoc:allow(CODE)` comment that matched no finding. A stale suppression hides nothing today, but it would silently hide the next finding on that line.

## What counts

- **What an allow covers:** a finding on its own line (a trailing comment) or on the line below.
- **Unused:** an allow that covered none is reported at its own line.

## Fix

Delete the comment, or move it to the line the finding is on.

This code is emitted after configuration is applied, so `--deny`, `--allow` and `[check]` do not accept it.
