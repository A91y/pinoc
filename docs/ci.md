# CI

## Installing pinoc

From 0.4.0, each GitHub release has prebuilt binaries attached for Linux (x86_64, aarch64) and macOS (x86_64, aarch64), so CI does not have to compile pinoc.

The repository is a GitHub Action that installs a pinned version:

```yaml
- uses: a91y/pinoc@v0.4.0
  with:
    version: 0.4.0            # pin it: a different pinoc can write a different IDL
    solana-version: <x.y.z>   # optional: also install this Agave release (solana, cargo build-sbf)
```

- **Checksums:** the action checks the binary against the release's SHA-256.
- **No prebuilt binary:** for an older version, or a platform without one, it falls back to `cargo install`.
- **Version check:** it confirms `pinoc --version` matches.
- **Outside Actions:** `cargo binstall pinoc --version <v>` uses the same binaries, and likewise compiles from source when there is none.

Pin the version. A different pinoc can write a different IDL or client, so a bump means regenerating the committed files in the same change.

## A workflow

Run from the program's directory:

```yaml
- uses: a91y/pinoc@v0.4.0
  with: { version: 0.4.0, solana-version: <x.y.z> }
- run: pinoc idl --check
  working-directory: program
- run: pinoc client generate --check --auto-install
  working-directory: program
- run: pinoc check --format github
  working-directory: program
- run: pinoc test --all-clusters
  working-directory: program
```

| Step | What it checks | Needs |
|---|---|---|
| `pinoc idl --check` | the committed IDL matches the source ([IDL](idl.md#keeping-a-committed-idl-current)) | `[idl] out_dir` |
| `pinoc client generate --check` | every client in `[client] languages` matches the committed IDL ([clients](clients.md#configured-clients-and---check)) | Node.js for codama clients. Run it in the checkout, not a copy, so prettier finds the repository's config. |
| `pinoc check --format github` | the lints, as annotations on the pull request ([`pinoc check`](check.md#output)) | |
| `pinoc test --all-clusters` | the suite once per `[clusters]` feature set ([testing](build-and-test.md#testing)) | `solana-version`, for `cargo build-sbf` |

The `--check` steps write nothing and exit nonzero on drift, listing each stale file. The fix is to run the same command without `--check` and commit the result.

## Code scanning

To send findings to the GitHub Security tab instead of, or as well as, annotations:

```yaml
- run: pinoc check --format sarif > pinoc.sarif || true
  working-directory: program
- uses: github/codeql-action/upload-sarif@v3
  with: { sarif_file: program/pinoc.sarif }
```

`|| true` lets the upload run when the check fails, so this step is not the gate; keep `pinoc check --format github` (or plain `pinoc check`) for that. Paths in both formats start at the repository root, so a crate below it is annotated on the right files.
