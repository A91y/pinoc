# pinoc documentation

| Page | Covers |
|---|---|
| [Projects, building and testing](build-and-test.md) | `pinoc init`, `build`, `test`, features, `[test]`, the build record, the entrypoint check, SBPF versions |
| [Clusters, deploying and keys](deploy.md) | `[clusters]`, `pinoc deploy`, the program keypair, `pinoc verify`, `pinoc keys` |
| [IDL generation](idl.md) | `pinoc idl`, shank and Codama programs, errors, constants, the padding warning |
| [Client generation](clients.md) | `pinoc client generate`, the shank and codama generators, TypeScript, `--check` |
| [`pinoc check`](check.md) | the lints, what is analysed, configuration, suppression, output formats |
| [Lint codes](lints/) | one page per code, with a flagged and a clean example |
| [CI](ci.md) | the setup action, prebuilt binaries, a workflow, code scanning |
| [Configuration](configuration.md) | every `Pinoc.toml` key |

Contributor notes on how each part is built are in [`src/check/`](../src/check/README.md), [`src/idl/`](../src/idl/README.md) and [`src/client_gen/`](../src/client_gen/README.md).
