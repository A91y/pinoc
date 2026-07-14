# IDL generation

`pinoc build` and `pinoc idl` extract a program's interface and write two files to `target/idl/` (override with `--out-dir`):

- **`<name>.json`** — shank's native IDL, extracted directly from the program's `Shank*` derive macros. This is the canonical file, and the one `pinoc client generate --generator shank` consumes.
- **`<name>.codama.json`** — a Codama-compatible IDL, consumed by `pinoc client generate --generator codama` and by any external Codama pipeline.

Extraction is powered by [shank](https://github.com/metaplex-foundation/shank), vendored into `pinoc` (no separate install). A failed extraction prints a warning; it never fails the build.

## The two paths to `.codama.json`

`<name>.codama.json` is produced one of two ways, chosen automatically:

- **Shim** (default for shank programs). Rewrites shank's native output so it round-trips through Codama's JS tooling: shank emits pubkey fields as `{"defined": "Address"}`, which `@codama/nodes-from-anchor` mishandles, so the shim rewrites them to the standard `"publicKey"` IDL type. The plain `<name>.json` is left untouched.
- **Native** (for Codama programs). When the program depends on `codama` and uses at least one of its Rust derive macros (`CodamaAccount`, `CodamaInstructions`, `CodamaErrors`, `CodamaType`, …), `pinoc` invokes Codama's own extractor and emits its IDL directly.

You can tell the outputs apart: native carries a top-level `"kind": "rootNode"`; the shim carries `"metadata": {"origin": "shank", …}` instead.

## Generator selection

The `.codama.json` path is resolved by, in precedence order:

1. the `--idl-generator` CLI flag (`shank` | `codama`),
2. `Pinoc.toml`'s `[idl] generator` (`auto` | `shank` | `codama`),
3. auto-detection (native if Codama macros are present, shim otherwise).

```bash
pinoc idl --idl-generator shank    # force the shim, even if Codama macros exist
pinoc idl --idl-generator codama   # force native extraction (errors if no Codama macros)
```

```toml
# Pinoc.toml
[idl]
generator = "auto"   # "auto" | "shank" | "codama"
```

## Program address

shank reads the program ID from the source. Programs that declare it via `declare_id!` work out of the box. Programs that use `Address::from_str_const` (to avoid the extra `pinocchio-pubkey`/`decode` dependency) don't expose the address to shank, so pass it explicitly:

```bash
pinoc idl   --program-id <ADDRESS>
pinoc build --program-id <ADDRESS>   # same override for the automatic post-build step
```

Instructions, accounts, and types still require shank's derive macros to appear in the IDL; `pinoc idl` does not infer them from unannotated code.

## Errors

shank only recognizes error enums deriving `thiserror::Error`. When none are found, `pinoc` falls back to scanning `src/` for a plain enum with a manual `impl From<X> for ProgramError`, synthesizing a message per variant from its name (`InvalidPda` → "Invalid Pda").

## Zero-copy padding lint

The generated client (de)serializes as packed borsh, while scaffolded programs read instructions and accounts zero-copy via `#[repr(C)]` + pointer casts. The two layouts agree only when the `#[repr(C)]` struct has no implicit alignment padding. `pinoc build` and `pinoc idl` warn when any `ShankAccount`/`ShankType` `#[repr(C)]` struct has padding (e.g. a `u64` after a `u8`). The fix is explicit `_padding: [u8; N]` fields; scaffolded structs also carry a compile-time `assert!(size_of::<T>() == …)` guard.

## Module map

| File | Responsibility |
| --- | --- |
| `mod.rs` | Entry point; extracts `<name>.json` and the error list, then routes `.codama.json` to the shim or native path per `resolve_generator`. |
| `codama.rs` | The shim: rewrites shank's IDL into Codama-compatible JSON. |
| `codama_native.rs` | Detects Codama derive macros and drives Codama's own extractor. |
| `manual_errors.rs` | Fallback error extraction for enums without `thiserror::Error`. |
| `padding_lint.rs` | Flags `#[repr(C)]` IDL structs with implicit padding. |
