# Client generation internals

How to use it is in [docs/clients.md](../../docs/clients.md). This page is for changing it.

- **The shank generator** (`shank/`) is pinoc's own Rust renderer. It mirrors the conventions of Codama's Rust renderer, so the two clients look alike.
- **The codama generator** (`codama/`) writes a small Node.js script into `<out-dir>/.pinoc-codama/` and runs it against packages installed from the lock files built into pinoc (`codama/npm/`). Changing a renderer version means changing those lock files, and generated clients may change with them.
- **`discriminants.rs`** finds enums with explicit, non-positional discriminants and rewrites every generator's output to use them. When a renderer's output is not in the expected shape, it fails rather than leave a wrong client.

## Module map

| Path | Responsibility |
| --- | --- |
| `mod.rs` | Dispatches to the `shank` or `codama` generator. |
| `shank/mod.rs` | Orchestrates the built-in Rust renderer. |
| `shank/instructions.rs` | Instruction builders. |
| `shank/accounts.rs` | Account (de)serialization and `fetch_*` helpers. |
| `shank/types.rs` | Defined types. |
| `shank/cpi.rs` | CPI variants (`XxxCpi` / `XxxCpiBuilder`). |
| `shank/manifest.rs` | The generated crate's `Cargo.toml`. |
| `shank/shared.rs` | Shared render helpers. |
| `discriminants.rs` | Finds enums with explicit, non-positional discriminants and makes every generated client use them. |
| `codama/mod.rs` | Drives the external Codama JS pipeline (Rust and TypeScript renderers). |
