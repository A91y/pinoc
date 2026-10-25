# IDL generation

`pinoc build` and `pinoc idl` extract a program's interface and write two files to `target/idl/`, or to [`[idl] out_dir`](configuration.md#idl). `pinoc idl --out-dir` overrides both.

- **`<name>.json`** is shank's native IDL, extracted from the program's `Shank*` derive macros. It is the canonical file, and the one `pinoc client generate --generator shank` reads.
- **`<name>.codama.json`** is a Codama-compatible IDL, read by `pinoc client generate --generator codama` and by any external Codama pipeline.

Extraction is powered by [shank](https://github.com/metaplex-foundation/shank), built into pinoc. A failed extraction in `pinoc build` prints a warning and never fails the build.

## Keeping a committed IDL current

A project that commits its IDL sets `out_dir` to the committed directory, so `pinoc build` keeps it current and `pinoc client generate` reads it from there:

```toml
[idl]
out_dir = "idl"
```

`pinoc idl --check` extracts into a temporary directory and compares. It writes nothing, lists each file that differs or is missing, and exits nonzero. See [CI](ci.md).

## The two paths to `.codama.json`

`<name>.codama.json` is produced one of two ways:

- **Shim** (the default for shank programs) rewrites shank's output so it round-trips through Codama's JS tooling. shank emits pubkey fields as `{"defined": "Address"}`, which `@codama/nodes-from-anchor` mishandles, so the shim rewrites them to the standard `"publicKey"` type. `<name>.json` is left untouched.
- **Native** (for Codama programs) runs Codama's own extractor and writes its IDL directly. It is chosen when the program depends on `codama` or `codama-macros` and uses at least one of the derives (`CodamaAccount`, `CodamaInstructions`, `CodamaErrors`, `CodamaType`, ...), bare or qualified as `codama::CodamaAccount`.

The outputs can be told apart: native has a top-level `"kind": "rootNode"`; the shim has `"metadata": {"origin": "shank", ...}`.

### Generator selection

In precedence order: the `--idl-generator` flag (`shank` | `codama`), then `[idl] generator` in `Pinoc.toml` (`auto` | `shank` | `codama`), then auto-detection (native when Codama macros are present, the shim otherwise).

```bash
pinoc idl --idl-generator shank    # the shim, even with Codama macros
pinoc idl --idl-generator codama   # native extraction; fails without Codama macros
```

### Which Codama crate to depend on

Either `codama` or `codama-macros` enables the native path. Both export the same derives; they differ in what they link.

- **`codama-macros`** is a proc-macro crate, compiled for the host and never linked into the program. The `.so` is byte-identical to a build without it. Use it with `nostd_panic_handler!()`, and import the derives from `codama_macros`.
- **`codama`** is a normal library that is not `#![no_std]`, so depending on it links `std`. That fails under `nostd_panic_handler!()` with `E0152: found duplicate lang item panic_impl`, and builds under `default_panic_handler!()` or `entrypoint!`.

The panic handler decides which one fits. `default_panic_handler!()` defines no handler of its own and relies on `std` being linked, so with only proc-macro dependencies it fails with "`#[panic_handler]` function required, but not found". Choosing `default_panic_handler!()` is choosing to link `std`. The `--with-example` scaffold does this, since it depends on `shank` (see [Shank and `no_std`](#shank-and-no_std)).

Detection reads `[dependencies]` and every `[target.'cfg(..)'.dependencies]` table, and matches a renamed dependency on its `package` (`macros = { package = "codama-macros", version = "0.9" }`). When a `Codama*` derive is present but neither crate is a dependency, `pinoc idl` takes the shim path and says so: `(Codama derives found, but neither `codama` nor `codama-macros` is a dependency)`.

### Codama versions

pinoc extracts with **codama 0.13.2**. A `#[codama(..)]` directive is parsed twice: by the program's `codama-macros` when the program compiles (the derives validate their attributes on the host, though they emit nothing), and by pinoc's extractor. Both have to recognise it.

- **A directive added after 0.9** needs a `codama-macros` release that has it. This covers `remaining_accounts(..)` for a variable account tail, `array(..)`, `link(..)` and the other type keywords, `display` and `export`, all of which need 0.13. With `codama-macros` 0.9.3 they fail the build with `error: unrecognized codama directive` or `unrecognized type`, even though `pinoc idl`, which does not compile the program, extracts them.
- **An older macro crate** extracts the same way as long as the source uses only directives it has. Releases so far have only added directives.
- **A macro crate newer than 0.13** can accept a directive pinoc's extractor does not have. That stops the extraction, and the rejected directive is reported with its name, file, line and source line.

`pinoc idl` prints a note whenever the program's `codama-macros` is a different minor release than the extractor, saying which case applies. Bumping `codama-macros` does not change what the program does. The `.so` keeps its length and behaviour, though the compiler may lay the code out differently, so builds across a bump need not be byte-identical.

codama 0.13 writes a newer IDL (spec 1.8) and leaves out lists that are empty, which the Rust renderer does not expect; an instruction with no accounts would render as `Vec::with_capacity(NaN + ..)`. pinoc writes those empty lists back into `<name>.codama.json`, so both renderers read it.

### Annotating for Codama

- **Array lengths must be literals.** Codama does not evaluate a `const` in an array size. A field typed `[Tier; TIER_COUNT]` with no attribute fails with "does not resolve to a `structFieldTypeNode`", and a const inside the attribute is rejected too.
  - Keep the const in the Rust type and give the IDL type a literal: `#[codama(type = array(number(u64), 4))]`.
  - Nothing ties that literal to the const, and a wrong one compiles and silently produces a wrong IDL, so keep a guard beside the field: `const _: () = assert!(TIER_COUNT == 4);`.
- **A type with its own `CodamaType` derive** is referenced with `link("name")` inside `array`, `option`, `set`, `map` or `tuple`: `#[codama(type = array(link("tier"), 4))]`. `defined(..)` is not a keyword.
- **A variable account tail** is `#[codama(remaining_accounts(argument("positions")))]` on the instruction, with optional `signer`, `writable` and `optional` flags that apply to the whole tail. It becomes a flat list input on the generated client. Codama has no node for a tail made of groups or of entries that differ.
- **Optional accounts in the middle of a list** work with `#[codama(optional_account_strategy = omitted)]`. The TypeScript client drops an omitted optional account and the ones after it move up, so one instruction can describe two layouts that differ by optional entries. A `remaining_accounts` tail is appended after that.

## Shank and `no_std`

`shank` has the same shape as `codama`: a normal library that re-exports the `shank_macro` proc-macro crate and is not `#![no_std]`. A program that uses a `Shank*` derive through `shank` links `std`, so it needs `default_panic_handler!()` or `entrypoint!`, and fails under `nostd_panic_handler!()` with `E0152`. An unused `shank` dependency is not linked, so the error only appears once a derive is used.

To keep `std` out, depend on **`shank_macro`** directly and import the derives from it (`shank_macro::ShankAccount`). It is host-only and leaves the `.so` unchanged. `pinoc idl` extracts the same IDL, since it reads derive names from source, whichever crate provides them.

## Program address

shank reads the program ID from source. `declare_id!` works as is. A program that uses `Address::from_str_const` (to avoid the extra `pinocchio-pubkey`/`decode` dependency) does not expose the address to shank, so pass it:

```bash
pinoc idl   --program-id <ADDRESS>
pinoc build --program-id <ADDRESS>
```

Instructions, accounts and types still need shank's derives to appear in the IDL; `pinoc idl` does not infer them from unannotated code.

## Errors

shank only recognizes error enums deriving `thiserror::Error`. When it finds none, pinoc scans `src/` for a plain enum with a manual `impl From<X> for ProgramError` and synthesizes a message per variant from its name (`InvalidPda` becomes "Invalid Pda").

**Codes are the values the program returns,** not the raw discriminants. pinoc evaluates what the `From` impl passes to `ProgramError::Custom`. It follows:
- a constant offset (`e as u32 + 6000`, a named or associated `const`, simple constant arithmetic);
- hops through a method on the enum (`e.code()`);
- a `match` that gives every variant a literal code.

**Which errors are converted:**
- **thiserror-derived errors** and natively extracted `CodamaErrors` get the same conversion, since shank and Codama both read codes from discriminants.
- **For Codama, it is decided per error:** a code still equal to the raw discriminant is converted, and any other code (set explicitly in the program) is kept.
- **An error whose name matches no variant** of the `From` enum is left alone.
- **Names are paired** on their lowercased alphanumerics, so `NotAMint` matches Codama's `notAmint`.
- **Every error left unconverted is named in a warning.** When the `From` body is none of the forms above, pinoc keeps the discriminants and warns, since the IDL codes may then not match the program's.

**Native extraction only sees enums deriving `CodamaErrors`.** When it finds none, pinoc fills `program.errors` in `<name>.codama.json` from `<name>.json`'s errors (thiserror-derived or the fallback above), so the native path never drops errors the shank path reports. Errors Codama extracted itself are never replaced. An error with no `#[error("..")]`, which Codama gives an empty message, gets its message from the same list.

**Error names.** Codama's conversion to camelCase does not keep a one-letter word or an acronym apart: `NotAMint` becomes `notAmint` and `HTTPTimeout` becomes `httptimeout`, which a client exports as `NOT_AMINT` and `HTTPTIMEOUT`. Set the name you want on the variant:

```rust
#[derive(CodamaErrors)]
pub enum MyError {
    #[codama(name = "notAMint")]
    NotAMint,
}
```

pinoc writes it to `<name>.codama.json` as spelled, keeping the variant's code and message; Codama on its own would re-case it.
- **Valid names:** the name must be camelCase.
- **Collisions:** one that collides with another variant is refused, because Codama would keep only one of the two errors without saying so.
- **Without `name`:** variants keep Codama's conversion.
- **Reporting:** `pinoc idl` prints how many names it took from `name`; a `name` it prints nothing about was left to Codama.

This applies to error variants only. `name` on an account, a type, a field or an argument is left as Codama writes it, re-cased (`#[codama(account(name = "notAMint"))]` gives `notAmint`), because those names are referred to from other nodes (links, PDA seeds, account defaults), and renaming one after extraction would break the references.

A variant with `#[codama(skip)]` is left out of `<name>.codama.json`, and a `name` on it names nothing. The other variants' codes do not move. `pinoc idl` lists the variants left out this way.

## Constants

Neither extractor can declare a program constant from source (Codama has a `constantNode` but no directive that builds one), so pinoc exports a `const` that carries a marker comment directly above it:

```rust
/// Open positions a trader may hold.
// pinoc:constant
pub const MAX_OPEN_POSITIONS: u8 = 8;

// pinoc:constant(u8)
pub const MAX_ATAS: usize = 12;

// pinoc:constant
pub const MAX_RESERVE_FLOOR: u64 = 100_000_000 * MILLION;
```

- **The value** is evaluated from the const's own expression: integer literals, `+ - * / <<`, casts, and other consts in the crate. Nothing is repeated, so it cannot drift from the program.
- **The type** is the declared one when it is a fixed-width integer (`u8` to `u128`, `i8` to `i128`). A `usize` has no wire type, so name one in the marker (`// pinoc:constant(u8)`); the value is checked against it.
- **Placement:** the marker goes directly above the `const`. When the const has doc comments or attributes (which become the constant's docs), the marker goes above those or anywhere between them and the `const`. Blank lines there are harmless.

Each constant is written to `constants` in `<name>.json` (`{name, type, value}`) and to `program.constants` in `<name>.codama.json` (a `constantNode`, named in camelCase).

**A marker that cannot be honoured is an error, not a skipped constant:**
- a type that is not an integer;
- a value that does not fit, or that pinoc cannot evaluate;
- a value beyond 2^53 (an IDL number is a JSON number, which a JavaScript client reads as a double);
- a marker that is not directly above a `const` (the error names the marker's file and line).

Only integers are supported.

The TypeScript client renders them (`constants/<program>.ts`, `export const MAX_ATAS: number = 12;`, `bigint` for 64-bit types). The Rust renderers do not render program constants.

## Zero-copy padding warning

The generated client (de)serializes as packed borsh, while scaffolded programs read instructions and accounts zero-copy through `#[repr(C)]` and pointer casts. The two layouts agree only when the struct has no implicit alignment padding. `pinoc build` and `pinoc idl` warn when a `#[repr(C)]` `ShankAccount`/`ShankType` struct has padding (a `u64` after a `u8`, say). The fix is explicit `_padding: [u8; N]` fields; scaffolded structs also carry a compile-time `assert!(size_of::<T>() == ..)` guard. `pinoc check` reports the same thing as [`ZC001-P`](lints/ZC001-P.md).
