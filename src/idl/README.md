# IDL generation

`pinoc build` and `pinoc idl` extract a program's interface and write two files to `target/idl/` (override with `--out-dir`):

- **`<name>.json`**: shank's native IDL, extracted directly from the program's `Shank*` derive macros. This is the canonical file, and the one `pinoc client generate --generator shank` consumes.
- **`<name>.codama.json`**: a Codama-compatible IDL, consumed by `pinoc client generate --generator codama` and by any external Codama pipeline.

Extraction is powered by [shank](https://github.com/metaplex-foundation/shank), vendored into `pinoc` (no separate install). A failed extraction prints a warning; it never fails the build.

## The two paths to `.codama.json`

`<name>.codama.json` is produced one of two ways, chosen automatically:

- **Shim** (default for shank programs). Rewrites shank's native output so it round-trips through Codama's JS tooling: shank emits pubkey fields as `{"defined": "Address"}`, which `@codama/nodes-from-anchor` mishandles, so the shim rewrites them to the standard `"publicKey"` IDL type. The plain `<name>.json` is left untouched.
- **Native** (for Codama programs). When the program depends on `codama` or `codama-macros` and uses at least one of the Rust derive macros (`CodamaAccount`, `CodamaInstructions`, `CodamaErrors`, `CodamaType`, …), written bare or qualified as `codama::CodamaAccount`, `pinoc` invokes Codama's own extractor and emits its IDL directly.

### Which Codama crate to depend on

Either `codama` or `codama-macros` in the program's manifest enables the native path. Both export the same derives. They differ in what they link:

- **`codama-macros`** is a proc-macro crate: compiled for the host, never linked into the program. Adding it and its derives leaves the `.so` byte-identical to a build without it. It is the one to use with `nostd_panic_handler!()`. Import the derives from `codama_macros`.
- **`codama`** is a normal library that is not `#![no_std]`, so depending on it links `std` into the program. That fails under `nostd_panic_handler!()` with `E0152: found duplicate lang item panic_impl`, and builds under `default_panic_handler!()` or `entrypoint!`.

The panic handler decides which one fits, and the two handlers are not interchangeable: `default_panic_handler!()` defines no handler of its own and relies on `std` being linked, so a program using it with only proc-macro dependencies fails with "`#[panic_handler]` function required, but not found". Choosing `default_panic_handler!()` is choosing to link `std`. The `--with-example` scaffold does this (it depends on `shank`, see [Shank and `no_std`](#shank-and-no_std)).

Detection reads the manifest's `[dependencies]` and every `[target.'cfg(..)'.dependencies]` table, and matches a renamed dependency on its `package` (`macros = { package = "codama-macros", version = "0.9" }`). When a `Codama*` derive is present but neither crate is a dependency, `pinoc idl` takes the shim path and says so: `(Codama derives found, but neither `codama` nor `codama-macros` is a dependency)`.

`pinoc` extracts with **codama 0.13.2**. A `#[codama(..)]` directive is parsed twice: by the program's `codama-macros` when the program compiles (the derives validate their attributes on the host, even though they emit nothing), and by pinoc's extractor when the IDL is generated. Both have to recognise it.

- **To use a directive added after 0.9**, the program must depend on a `codama-macros` release that has it: `remaining_accounts(..)` for a variable account tail, `array(..)`, `link(..)` and the other type keywords, `display`, `export` all need 0.13. With `codama-macros` 0.9.3 they fail the build with `error: unrecognized codama directive` or `unrecognized type`, although `pinoc idl`, which does not compile the program, extracts them.
- **An older macro crate extracts the same way** as long as the source uses only directives it has; releases so far have only added directives.
- **A macro crate newer than 0.13** can accept a directive pinoc's extractor does not have, which stops the extraction. The rejected directive is reported with its name, file, line, and the source line.

`pinoc idl` prints a note whenever the program's `codama-macros` is a different minor release than the extractor, saying which of the two cases applies. Bumping `codama-macros` does not change what the program does: the `.so` keeps its length and behaviour, though the compiler may lay the code out differently, so two builds across a bump need not be byte-identical.

Things to know when annotating:

- **Array lengths must be literals.** Codama does not evaluate a `const` in an array size. A field typed `[Tier; TIER_COUNT]` with no attribute fails with "does not resolve to a `structFieldTypeNode`", and a const inside the attribute is rejected too. Keep the const in the Rust type and give the IDL type a literal: `#[codama(type = array(number(u64), 4))]`. Nothing ties that literal to the const, and a wrong one compiles and silently produces a wrong IDL, so keep a guard beside the field: `const _: () = assert!(TIER_COUNT == 4);`.
- **A type with its own `CodamaType` derive is referenced with `link("name")`** inside `array`, `option`, `set`, `map` or `tuple`: `#[codama(type = array(link("tier"), 4))]`. `defined(..)` is not a keyword.
- **A variable account tail** is `#[codama(remaining_accounts(argument("positions")))]` on the instruction, with optional `signer`, `writable` and `optional` flags that apply to the whole tail. It becomes a flat list input on the generated client; Codama has no node for a tail made of groups or of entries that differ.
- **Optional accounts in the middle of a list** work with `#[codama(optional_account_strategy = omitted)]`: the TypeScript client drops an omitted optional account and the ones after it move up, so one instruction can describe two account layouts that differ by optional entries. A tail declared with `remaining_accounts` is appended after that.

codama 0.13 writes a newer IDL (spec 1.8) and leaves out lists that are empty, which the pinned Rust renderer does not expect: an instruction with no accounts rendered as `Vec::with_capacity(NaN + ..)`. `pinoc` writes those empty lists back into `<name>.codama.json`, so the file keeps the shape earlier releases produced and both the Rust and TypeScript renderers read it. For a program that uses no new directive, the IDL differs from pinoc 0.3.1's only in the spec version and a `display` hint on discriminator fields, and the generated clients are identical.

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

## Shank and `no_std`

`shank` has the same shape as `codama`: a normal library that re-exports the `shank_macro` proc-macro crate and is not `#![no_std]`. A program that uses a `Shank*` derive through `shank` links `std`, so it needs `default_panic_handler!()` or `entrypoint!`, and fails under `nostd_panic_handler!()` with `E0152: found duplicate lang item panic_impl`. (An unused `shank` dependency is not linked, so the error only appears once a derive is used.)

To keep `std` out, depend on **`shank_macro`** directly and import the derives from it (`shank_macro::ShankAccount`). It is host-only, leaves the `.so` unchanged, and `pinoc idl` extracts the same IDL, since extraction reads the derive names from source and does not depend on which crate provides them.

## Program address

shank reads the program ID from the source. Programs that declare it via `declare_id!` work out of the box. Programs that use `Address::from_str_const` (to avoid the extra `pinocchio-pubkey`/`decode` dependency) don't expose the address to shank, so pass it explicitly:

```bash
pinoc idl   --program-id <ADDRESS>
pinoc build --program-id <ADDRESS>   # same override for the automatic post-build step
```

Instructions, accounts, and types still require shank's derive macros to appear in the IDL; `pinoc idl` does not infer them from unannotated code.

## Errors

shank only recognizes error enums deriving `thiserror::Error`. When none are found, `pinoc` falls back to scanning `src/` for a plain enum with a manual `impl From<X> for ProgramError`, synthesizing a message per variant from its name (`InvalidPda` → "Invalid Pda").

Codes are the values the program returns, not the raw discriminants: `pinoc` evaluates what the `From` impl passes to `ProgramError::Custom`. It follows a constant offset (`e as u32 + 6000`, a named or associated `const`, simple constant arithmetic), one or more hops through a method on the enum (`e.code()`), and a `match` that gives every variant a literal code. The same conversion is applied to thiserror-derived errors and to natively extracted `CodamaErrors`, since shank and Codama both read codes from discriminants; For Codama this is decided per error: a code still equal to the raw discriminant is converted, any other code (set explicitly in the program) is kept, and an error whose name matches no variant of the `From` enum is left alone. Every error left unconverted is named in a warning. Names are paired on their lowercased alphanumerics, so `NotAMint` matches Codama's `notAmint`. When the `From` body is none of these, `pinoc` keeps the discriminants and prints a warning, since the IDL codes may then not match the program's.

Native Codama extraction only sees enums deriving `CodamaErrors`. When it finds none, `pinoc` fills `program.errors` in `<name>.codama.json` from the error list of `<name>.json` (thiserror-derived or the manual fallback above), so choosing the native path never drops errors the shank path reports. Errors Codama extracted itself are never replaced. Codama emits an empty message for an error without an `#[error("..")]` attribute; those are filled from the same list.

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
