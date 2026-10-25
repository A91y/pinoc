# Configuration

`Pinoc.toml`, at the program's root, is optional; `pinoc config init` creates one. Without it, every command uses its defaults, and `pinoc deploy` takes the cluster and wallet from `solana config get`. Command-line flags always override the file.

When the file exists:
- **Required section:** `[provider]` with `cluster` and `wallet`.
- **Refused:** a misspelt section, or a misspelt key under `[build]`, `[test]`, `[clusters.*]` or `[check]`, stops the command and names it, so a typo cannot read as a setting that does nothing.
- **Older releases:** releases before 0.4 refuse the `[test]` and `[clusters]` sections, so a project that uses them needs 0.4 or later everywhere it runs.

```toml
[provider]
cluster = "localhost"
wallet = "~/.config/solana/id.json"
```

This page lists every key once. The pages linked from each section explain the behaviour.

## `[provider]`

Used by [`pinoc deploy` and `pinoc verify`](deploy.md).

| Key | Default | Meaning |
|---|---|---|
| `cluster` | (required) | A `[clusters]` name, or a URL or moniker for `solana --url`. Overridden by `--cluster`. |
| `wallet` | (required) | The payer keypair. Overridden by `--wallet`, and by a cluster's own `wallet`. |
| `program_keypair` | `target/deploy/<name>-keypair.json` | The program's own keypair, used as `--program-id`. Overridden by `--program-keypair`. |
| `upgrade_authority` | the wallet | Passed as `--upgrade-authority`. Overridden by the flag, and by a cluster's own `upgrade_authority`. |

## `[clusters.<name>]`

A named cluster whose build needs cargo features. Used by `pinoc build`, `pinoc test`, `pinoc deploy` and `pinoc verify`; see [Clusters](deploy.md#clusters).

| Key | Default | Meaning |
|---|---|---|
| `url` | the entry's name | What `solana --url` is given, so `[clusters.devnet]` reaches devnet with no `url`. |
| `features` | none | The cargo features an artifact for this cluster is built with. |
| `wallet` | `[provider] wallet` | The payer for this cluster. |
| `upgrade_authority` | `[provider] upgrade_authority` | The upgrade authority for this cluster. |

## `[build]`

| Key | Default | Meaning |
|---|---|---|
| `arch` | the toolchain's | The SBPF version (`v0` to `v4`) that `pinoc build` and `pinoc test` pass to `cargo build-sbf --arch`. Overridden by `--arch`. See [SBPF version](build-and-test.md#sbpf-version). |

## `[test]`

See [Test configuration](build-and-test.md#test-configuration).

| Key | Default | Meaning |
|---|---|---|
| `features` | none | Features added to every `cargo test`, before `--features`. |
| `programs` | none | Directories of other SBF programs the tests load, each with its own `Cargo.toml`, built with the same `--arch`. |

## `[idl]`

See [IDL generation](idl.md).

| Key | Default | Meaning |
|---|---|---|
| `generator` | `"auto"` | How `<name>.codama.json` is made: `"auto"`, `"shank"` (the shim) or `"codama"` (native extraction). Overridden by `--idl-generator`. |
| `out_dir` | `target/idl` | Where `pinoc build` and `pinoc idl` write the IDL, and where `pinoc client generate` reads it. Overridden by `--out-dir` / `--idl-dir`. |

## `[client]`

See [Client generation](clients.md).

| Key | Default | Meaning |
|---|---|---|
| `languages` | `["rust"]` | The clients a bare `pinoc client generate` renders: `"rust"`, `"ts"`. Overridden by `--language`. |
| `generator` | prompt, or the detected one when not interactive | The Rust client's generator, `"shank"` or `"codama"`. Overridden by `--generator`. TypeScript always uses codama. |
| `shank_out_dir` | `clients/rust-shank` | The shank Rust client's directory. |
| `codama_out_dir` | `clients/rust-codama` | The codama Rust client's directory. |
| `out_dir` | none | One directory for whichever Rust generator runs. Warns on each run, since the two generators would overwrite each other there. The per-generator keys win over it. |
| `ts_out_dir` | `clients/ts` | The TypeScript client's directory. |

`--out-dir` overrides all of these for one run.

## `[check]`

See [`pinoc check`](check.md#configuration).

| Key | Default | Meaning |
|---|---|---|
| `deny` | none | Codes that fail the check. `*` or `all` means every code. |
| `warn` | none | Codes reported as advisory. |
| `allow` | none | Codes not reported. |
| `confidence_threshold` | `"likely"` | Findings weaker than this are hidden: `"heuristic"`, `"likely"` or `"definite"`. |
| `authority_names` | none | More names `ACC002-P` treats as an authority that must sign, besides `authority`, `admin`, `auth` and names ending in `_authority` or `_auth`. |

An entry that is not a lint code, or a threshold that is not one of the three values, stops the command.
