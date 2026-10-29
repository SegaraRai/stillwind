# Development

This repository contains the Rust compiler, JavaScript host services, build
integrations, and developer tools. The workspace uses pnpm through Vite+.
Tasks are defined in each package's `vite.config.ts#run.tasks`.

## Setup and verification

Install Vite+, Rust, and the platform linker. The required Rust version is declared
in [Cargo.toml](Cargo.toml). Vite+ uses the pnpm version selected by the workspace.

Run from the repository root:

```sh
vp i --frozen-lockfile
vp run verify
```

`verify` runs `check` and `test`. Browser, WASM, framework acceptance, and timing
checks have separate tasks and prerequisites. See [Tests and verification](docs/test-inventory.md)
for the task inventory, generated files, cache behavior, and fixture workflows.
Run generation tasks explicitly and review their output before comparing fixtures.

Package build tasks build their dependencies through the task graph. Applications
that use the workspace integration declare `dependsOn: ["@stillwind/integration#build"]`
for their development and build tasks. This prerequisite builds the integration's
`dist/` files before the application loads them.

Before you publish `stillwind`, run `vp run stillwind#prepare-publish`. It builds the
package and the website, and it copies the website's Markdown pages into
`packages/core/docs/`, which git ignores.

## Repository layout

| Directory     | Responsibility                                                                 |
| ------------- | ------------------------------------------------------------------------------ |
| `crates/`     | Compiler, CSS design systems, source models, project sessions, and bindings    |
| `packages/`   | Public application APIs, build integrations, host services, and editor tooling |
| `plugins/`    | Optional utility plugins                                                       |
| `apps/`       | Website, playground, fixture viewer, and design tools                          |
| `e2e/`        | Browser acceptance across frameworks and bundlers                              |
| `fixtures/`   | Reviewed compiler output and Tailwind compatibility cases                      |
| `benchmarks/` | Explicit performance measurements and budgets                                  |
| `tools/`      | Generated data, fixtures, and shared task configuration                        |

Publishable packages use `stillwind`, `@stillwind/*`, or `prettier-plugin-stillwind`.
Private applications, acceptance fixtures, and tools use `@stillwind-internal/*`.
The root package is `@stillwind-internal/root`.

README files describe package usage. Internal README files state their role and
link to the relevant contracts. Keep repository workflows in this document,
package-specific development details in `DEVELOPMENT.md`, and shared contracts in
`docs/`. Record unresolved design and validation work in [docs/TODO.md](docs/TODO.md).

## Contracts and development guides

| Document                                                      | Responsibility                                                                |
| ------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| [Architecture](docs/crate-architecture.md)                    | Crate ownership, service entrypoints, diagnostics, and the Node binding       |
| [Macros](docs/macro-contract.md)                              | Accepted inputs, purity, composition, refs, animations, and custom properties |
| [CSS ordering](docs/css-ordering.md)                          | Declaration, module, and delivery precedence                                  |
| [CSS emission](docs/css-emission.md)                          | Linking, output slots, explicit values, and minification                      |
| [Generated identifiers](docs/generated-identifiers.md)        | Identity, naming callbacks, escaping, and collisions                          |
| [Runtime](docs/runtime.md)                                    | Lowering and framework materialization                                        |
| [Analysis](docs/standalone-analysis.md)                       | Configuration, analysis services, lint rules, and editor contracts            |
| [SFCs](docs/sfc-type-checking.md)                             | Projection, checker facts, and source coordinates                             |
| [Host services](packages/host/DEVELOPMENT.md)                 | Module ownership and editor preview lifetimes                                 |
| [Build integrations](packages/integration/DEVELOPMENT.md)     | Adapter lifetimes, transport, and verification                                |
| [VS Code extension](packages/vscode-extension/DEVELOPMENT.md) | Desktop/Web bundles and VSIX packaging                                        |
| [Browser acceptance](e2e/DEVELOPMENT.md)                      | Fixture execution, manifests, and artifacts                                   |
| [Benchmarks](benchmarks/DEVELOPMENT.md)                       | Workloads and measurement boundaries                                          |

Private applications document their local workflows beside their sources:
[Website](apps/website/DEVELOPMENT.md), [Playground](apps/play/DEVELOPMENT.md), and
[Shape Lab](apps/logo-editor/DEVELOPMENT.md).
