# DISC001-P `wrong-discriminator`

deny · definite · DISC (discriminator constants)

A marked discriminator constant whose bytes are not the leading bytes of the SHA-256 of the preimage it names: the Anchor convention, `event:Name`, `account:Name`, `global:snake_name`.

## What counts

Only marked constants are checked. A crate with no marker gets no finding.

- **Single form:** `// pinoc:discriminator("<preimage>")` above a `const` byte array.
- **Table form:** `// pinoc:discriminators("<prefix>", Enum)` above a `const` array of byte arrays.
  - Row `i` is checked against the variant whose discriminant is `i` (the declared value, or one more than the previous variant's), so a variant added, removed or reordered without its row is reported.
  - A third argument, `snake_case`, hashes the variant name in snake case.
- **Length:** each row's own length, 1 to 32 bytes.
- **Uncheckable markers:** reported with the reason. Examples: the enum is not in the file, the row and variant counts differ, a discriminant is not an integer literal, no variant has a row's index, a value is not a byte array literal.
- **Placement:** the finding is anchored at the wrong row.
- **Marker names:** `pinoc:discriminator` and `pinoc:discriminators` are told apart by the whole name.

## Limits

The table's enum must be in the same file as the table. The lint runs per file, and an enum looked up by name across modules could name two enums.

## Fix

The finding gives the right bytes.

## Flagged

```rust
pub enum EventKind {
    Deposit,
}

// pinoc:discriminators("event:", EventKind)
const DISCRIMINATORS: [[u8; 8]; 1] = [[0, 0, 0, 0, 0, 0, 0, 0]];
```

## Not flagged

```rust
pub enum EventKind {
    Deposit,
}

// pinoc:discriminators("event:", EventKind)
const DISCRIMINATORS: [[u8; 8]; 1] = [[62, 205, 242, 175, 244, 169, 136, 52]];
```
