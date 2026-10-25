# Projects, building and testing

## Creating a project

`pinoc init <name>` produces a blank, buildable program with one no-op instruction, a `Pinoc.toml`, and a program keypair. It initializes git unless given `--no-git`.

```
my_app/
├── Cargo.toml
├── Pinoc.toml
├── src/lib.rs
└── target/deploy/my_app-keypair.json
```

`--with-example` scaffolds a worked PDA-account program instead, annotated so `pinoc idl` works from the start:

```
my_app/
├── src/
│   ├── lib.rs
│   ├── entrypoint.rs
│   ├── errors.rs
│   ├── instructions/{mod.rs, initialize.rs}
│   └── states/{mod.rs, state.rs, utils.rs}
├── tests/tests.rs
└── target/deploy/my_app-keypair.json
```

Other project commands:
- `pinoc add <package>` adds a Pinocchio package, and `pinoc search [query]` lists them.
- `pinoc clean` removes build artifacts but keeps keypairs. `--no-preserve` removes everything.
- `pinoc config init` creates a `Pinoc.toml` in an existing project.

## Building

`pinoc build` runs `cargo build-sbf`, checks the artifact, records what it built, and regenerates the [IDL](idl.md).

- `--features <FEATURES>` (`-F`) activates cargo features, repeatable or comma-separated like cargo's own flag. `pinoc build` uses them as given: they describe the artifact you asked for.
- `--cluster <name>` adds the features of a [`[clusters]`](deploy.md#clusters) entry.
- `--arch <v0..v4>` sets the [SBPF version](#sbpf-version).
- `--program-id <ADDRESS>` sets the IDL's program address, for a program that does not call `declare_id!`.
- `--idl-generator <shank|codama>` forces [how `<name>.codama.json` is made](idl.md#generator-selection).

A failed IDL extraction is printed as a warning and does not fail the build.

## Testing

`pinoc test` runs `cargo build-sbf`, then `cargo test`. SVM tests (mollusk-svm, litesvm, solana-program-test) load `target/deploy/*.so`, which `cargo test` does not build, so without the build step they silently run against whatever was built last.

**Features.** `--features` applies to both steps, except a feature that enables `no-entrypoint` (such as the scaffold's `test-default`). That one is left out of the build, because `no-entrypoint` compiles the program out of the SBF artifact. When the build needs other features than the tests, set them with `--build-features`, which is used as given:

```bash
pinoc test --features test-default                                  # build: none, tests: test-default
pinoc test --features test-default,devnet --build-features devnet
pinoc test --features test-default --build-features ""              # build with no features
```

**Running part of the suite.** Everything after `--` goes to `cargo test` unchanged, after the features:

```bash
pinoc test --features test-default -- --test client_roundtrip
pinoc test --features test-default -- parses_the_fixture -- --exact --nocapture
```

**Other flags:**
- `--cluster <name>` builds and tests with a [`[clusters]`](deploy.md#clusters) entry's features.
- `--all-clusters` runs the suite once per distinct `[clusters]` feature set, reports every run, and fails if any failed.
- `--no-build` skips the SBF build; see [What `--no-build` tests](#what---no-build-tests).
- `--arch` sets the [SBPF version](#sbpf-version).

### Test configuration

`[test]` in `Pinoc.toml` holds what every run needs, so `pinoc test` alone runs the whole suite:

```toml
[test]
features = ["test-default"]        # added to every cargo test, before --features
programs = ["tests/mock_venue"]    # other SBF programs the tests load
```

Each entry in `programs` is a directory with its own `Cargo.toml`, such as a stand-in for a program the one under test calls. `pinoc test` builds each with `cargo build-sbf` in that directory and the program's `--arch`, after the program's own build and before the tests. `--no-build` skips them too.

### The build record

`pinoc build` and `pinoc test` write `target/deploy/<name>.build.json` beside the artifact:

| Field | Meaning |
|---|---|
| `features` | the features it was built with |
| `arch` | the `--arch` it was built with, or `null` |
| `cluster` | the `[clusters]` entry, when built with `--cluster` |
| `len`, `hash` | its length and FNV-1a hash, to notice it being replaced |
| `sha256` | its SHA-256, as `sha256sum` and `pinoc verify` print it |

The format is stable, so a test harness can read it to tell which build it is loading. [`pinoc deploy`](deploy.md#clusters) and [`pinoc verify`](deploy.md#verifying-a-deploy) read it too.

### What `--no-build` tests

`pinoc test --no-build` reads the build record and stops if the artifact was built with other features or another arch than this run would have used. Tests run against the wrong artifact fail in ways that read as bugs in the program:

```text
Error: target/deploy/prog.so was built with --features devnet, but this run would build it with no features. Drop --no-build to rebuild it, or pass --build-features "devnet" if that is the artifact to test.
```

With `--no-build`, `--build-features` and `--arch` say which artifact is expected. When there is no record, or the artifact is not the one it describes (built by `cargo build-sbf` directly, say), pinoc says it cannot tell what the artifact is and runs the tests.

## Entrypoint check

`pinoc build`, `pinoc test` and `pinoc deploy` refuse an SBF artifact with no entrypoint: one whose ELF entry address is outside its executable code, the rule the SBF loader enforces. That is what `no-entrypoint` produces. The artifact builds, but it contains no program, so no test can load it and a deploy would pay rent for nothing.

The error names the cause: a build feature, the `default` feature, or a program with no entrypoint. `pinoc build` and `pinoc test` delete the bad artifact; `pinoc deploy` checks whatever it is about to upload, however it was built.

## SBPF version

`cargo build-sbf` builds for the SBPF version given by `--arch`, or for its own default (`v0` in cargo-build-sbf 4.3). `pinoc build` and `pinoc test` pass `--arch` only when one is set, with the flag or in `Pinoc.toml`:

```toml
[build]
arch = "v3"
```

The flag overrides the file. Setting it in the file keeps `pinoc build` and `pinoc test` on the same version; with the flag alone, a later `pinoc test` without it rebuilds for the toolchain's default.

Which versions a cluster accepts is decided by two feature gates: one enables SBPFv3, and SIMD-0500 disables deployment of v0, v1 and v2. `solana-test-validator` 4.3 starts with both active, so it deploys only v3. `pinoc deploy` [checks the artifact against them](deploy.md#deploying) before uploading.
