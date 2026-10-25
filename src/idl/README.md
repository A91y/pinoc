# IDL generation internals

How to use it is in [docs/idl.md](../../docs/idl.md). This page is for changing it.

- **Two outputs, one source.** `mod.rs` always extracts `<name>.json` with shank first, then routes `<name>.codama.json` to the shim (`codama.rs`) or to Codama's own extractor (`codama_native.rs`), as `resolve_generator` decides.
- **Corrections after extraction.** Several things pinoc fixes after an extractor runs, because the extractor or a renderer gets them wrong. Each is applied in one place, so a fix to an upstream tool can be undone in one place:
  - error codes, through the `From` impl;
  - empty lists codama 0.13 omits;
  - error names given with `#[codama(name)]`.
- **Markers.** `// pinoc:constant` uses the shared marker helper in `src/markers.rs`, which `DISC001-P` also uses, so "directly above a const" means one thing.

## Module map

| File | Responsibility |
| --- | --- |
| `mod.rs` | Entry point; extracts `<name>.json` and the error list, then routes `.codama.json` to the shim or native path per `resolve_generator`. |
| `codama.rs` | The shim: rewrites shank's IDL into Codama-compatible JSON. |
| `codama_native.rs` | Detects Codama derive macros and drives Codama's own extractor. |
| `manual_errors.rs` | Fallback error extraction for enums without `thiserror::Error`. |
| `constants.rs` | Finds `// pinoc:constant` markers and evaluates the marked consts. |
| `padding_lint.rs` | Flags `#[repr(C)]` IDL structs with implicit padding. |
