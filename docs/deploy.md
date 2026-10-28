# Clusters, deploying and keys

## Clusters

A program whose build differs per cluster (venue program ids behind a `devnet` feature, say) names its clusters in `Pinoc.toml`:

```toml
[clusters.mainnet]
url = "mainnet-beta"

[clusters.devnet]
features = ["devnet"]                    # url defaults to the name

[clusters.localnet]
url = "localhost"
features = ["devnet"]
wallet = "~/.config/solana/local.json"   # optional, overrides [provider] wallet
```

Every key is in [Configuration](configuration.md#clustersname).

- **`pinoc build --cluster devnet`** builds with the entry's features, plus any `--features`, and records the cluster in the [build record](build-and-test.md#the-build-record).
- **`pinoc test --cluster devnet`** builds and tests with them; `[test] features` and `--features` are added.
- **`pinoc test --all-clusters`** runs the suite once per distinct feature set. Here that is twice, since `devnet` and `localnet` share one.
- **`pinoc deploy --cluster devnet`** deploys to the entry's `url`. It refuses an artifact whose build record shows it was built for another cluster: one missing a feature this cluster builds with, or carrying a feature only another cluster builds with.

```text
Error: target/deploy/prog.so was built with --features devnet (for cluster `devnet`), which is not a build for cluster `mainnet`: `devnet` is a feature of cluster `devnet`. Rebuild it with `pinoc build --cluster mainnet`.
```

**How the cluster is found:**
- **By name or `url`:** `--cluster`, or `[provider] cluster` when the flag is absent, finds an entry by name or by its `url`. With `[provider] cluster = "localhost"`, a bare `pinoc deploy` is checked as `localnet`.
- **No entry:** a value that names no entry goes to `solana --url` unchecked.
- **No build record:** an artifact built outside pinoc is reported and deployed unchecked.

## Deploying

`pinoc deploy` uploads `target/deploy/<name>.so`, the package's own artifact, with `solana program deploy`. It uses the program's keypair as `--program-id`, so the program lands at the address it declares and upgrades in place.

**The program keypair** is `--program-keypair`, else `[provider] program_keypair`, else `target/deploy/<name>-keypair.json`.
- **Refused if it does not match:** a keypair that holds another address than the program's `declare_id!` (or `const ID`) is refused. This matters on a fresh checkout: `cargo build-sbf` writes a new random keypair at the default path when there is none, and deploying with it would put the program at an address nothing refers to. Point `program_keypair` at the real keypair, which is usually kept out of the repository.
- **No keypair:** pinoc passes the declared address. That upgrades a program that already exists; a first deploy needs the keypair, and `solana` says so.
- **No keypair and no declared address:** refused, rather than deploying to a random address.

```toml
[provider]
cluster = "localhost"
wallet = "~/.config/solana/id.json"
program_keypair = "../keys/program.json"
upgrade_authority = "~/.config/solana/admin.json"   # optional
```

**Before uploading,** `pinoc deploy` refuses:
- an artifact with no entrypoint (see [Entrypoint check](build-and-test.md#entrypoint-check));
- an artifact built for another cluster (see [Clusters](#clusters));
- an artifact the cluster would reject for its SBPF version (see [SBPF version](build-and-test.md#sbpf-version)). pinoc reads the version from the ELF header and asks the cluster for its two SBPF feature gates (`solana feature status`). If the cluster would reject it, pinoc stops and names the `--arch` to rebuild with. If the cluster cannot be asked, the deploy goes ahead unchecked.

**Flags:** `--cluster`, `--wallet`, `--program-keypair` and `--upgrade-authority` override `Pinoc.toml`. Without a `Pinoc.toml`, the cluster and wallet come from `solana config get`.

## Verifying a deploy

`pinoc verify [--cluster <name>] [--program-id <ADDRESS>]` downloads the deployed program (`solana program dump`) and compares it with the local artifact.
- **Cluster:** with a `[clusters]` entry, an artifact whose build record says it was built for another cluster is refused before anything is downloaded, the same check `pinoc deploy` makes.
- **Padding:** the zero-filled space a program account has past the program is allowed for.
- **Output:** both SHA-256 hashes, and, when the build record matches the artifact, the features and cluster it was built with. The on-chain hash covers the program as its ELF headers give its length, so it is the same on every run whatever the local file is, and can be quoted in a deploy record.
- **Exit code:** nonzero when they differ, so it can follow every deploy or upgrade.

The address is the one declared under `src/` unless `--program-id` is given.

## Keys

```bash
pinoc keys list   # program keypairs in target/deploy and their addresses
pinoc keys sync   # rewrite the program ID in source to match the keypair
```

`keys sync` finds the program's own ID declaration anywhere under `src/` (`declare_id!`, or a `const ID`) and updates it in place. Use it for a new program that should take the keypair's address. For an existing program, fix the keypair instead.
