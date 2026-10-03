# Tests and verification

Tasks are defined in each workspace's `vite.config.ts`. Run them with Vite+.

| Root task               | Scope                                                                                                    |
| ----------------------- | -------------------------------------------------------------------------------------------------------- |
| `vp run check`          | Formatting, lint, TypeScript checks, Cargo formatting, and Clippy                                        |
| `vp run test`           | Rust workspace tests and ordinary JS tests in packages, tools, Play, and Shape Lab                       |
| `vp run fixtures:check` | Stillwind project outputs, Tailwind expectations, Webref data, and Tailwind ordering data                |
| `vp run verify`         | All three tasks above                                                                                    |
| `vp run verify:compat`  | Tailwind differential comparison, property ordering, numeric domains                                     |
| `vp run verify:browser` | CSS cascade, rendered filters, and ref scopes in Vitest Browser Mode                                     |
| `vp run verify:wasm`    | WASM build, root loader contracts, packed-package fallback, production bundling, and Vitest Browser Mode |

The root `test.projects` configuration registers nine ordinary JS projects.
`vp run test:js` builds their dependencies and runs them together. Use
`vp test --run --project host` to select one project after its dependencies have
been built. `vp test list --filesOnly` lists the selected files and project names.
Each referenced config declares its root and name.

`--mode browser` selects the Tailwind and CSS Browser Mode projects.
`--mode wasm` selects the Node WASI contracts, package and bundle checks, and the
WASI Browser Mode project. The verification tasks build and collect the required
artifacts before running those projects. Browser Mode uses the Playwright provider
with installed Chrome by default. Set `STILLWIND_BROWSER` to another Chromium
executable when needed. Both environments require the `wasm32-wasip1-threads`
Rust target and its build toolchain. These environments are separate from ordinary
tests. A requested environment's failure is reported rather than silently skipped.

Vite+ 1.0.0 bundles Vitest 5.0.1. The Vitest override and Browser Mode provider use
that same version. Browser tests run in the browser; Tailwind's global setup
compiles the expected and actual CSS in Node and provides it to those tests.
Framework E2E suites start separate applications and retain their Playwright harness.

## Test responsibilities

- Rust unit tests belong beside their implementation, including local `tests.rs`
  modules. Public API and CLI process tests belong in the owning crate's `tests/`.
- JS tests use `.test.ts` and belong beside the module they exercise. Use another
  extension only when the test needs its module or execution semantics.
- Integration tests that exercise several modules together belong in the owning
  package's `tests/` directory. A public API alone does not justify a separate
  test directory. Compile-only type contracts use `.ts` and run under `typecheck`.
- JS suites import from `vite-plus/test` and run with `vp test --run`. Runtime
  tests participate in TypeScript checks. Compile-only type assertions run under
  `typecheck`, including framework props and native binding declaration checks.
- Assertions use Vitest `expect`, including thrown errors and rejected promises.
  Unexpected branches and mock calls use `expect.fail`. A mock may throw an error
  when that error is an input to the behavior being tested.
- Native and WASM suites register the same binding contract with different entry
  points and execute in separate processes.
- Framework production builds run in the integration package's ordinary suite.
  Browser behavior uses the dedicated browser task. The independent module-graph
  experiments keep their own workspace and explicit acceptance commands.
- Project fixtures under `fixtures/projects/` record generated JS, CSS,
  diagnostics, and source mappings. Their expected files and the fixtures viewer
  catalog are produced by the same runner.

Tests protect current observable behavior. A test's reason to exist is the
contract it verifies, rather than the refactor or audit that first exposed it.
Configuration inventories and lists of forbidden former implementations are not
runtime contracts.

## Tool responsibilities

| Directory         | Responsibility                                                                          |
| ----------------- | --------------------------------------------------------------------------------------- |
| `tools/css/`      | Generate and check the Webref data used by CSS validation                               |
| `tools/projects/` | Compile project fixtures and check or refresh expected files and the viewer catalog     |
| `tools/tailwind/` | Upstream fixture generation, comparison, ordering, browser checks, and coverage reports |
| `tools/support/`  | Input construction and CSS loading shared by verification consumers                     |

Tailwind comparison adapters live beside the comparison that uses them. The CSS
loader used by integration tests is available as `@stillwind/tools/stillwind-css`.
Declaration-order comparison has no dependency on the native compiler; compiling
fixture candidates belongs to its separate fixture verifier.

Run a focused tool task with, for example,
`vp run @stillwind/tools#tailwind:parity` or
`vp run @stillwind/tools#tailwind:property-order`.
`tailwind:inventory` writes a coverage report, and `tailwind:analyze` prints a
difference analysis. These reports are explicit operations rather than ordinary
test prerequisites.

## Updating generated files

| Task                                                | Updates                                                |
| --------------------------------------------------- | ------------------------------------------------------ |
| `vp run fixtures:refresh`                           | Stillwind project expected files and viewer catalog    |
| `vp run @stillwind/tools#tailwind:fixtures:refresh` | Tailwind fixture expectations                          |
| `vp run webref:generate`                            | Rust Webref property data                              |
| `vp run @stillwind/tools#tailwind:order:generate`   | Rust Tailwind property order and utility-order fixture |

Check tasks leave tracked files unchanged. Review generated changes before
accepting them. Tasks that update files, use native binaries, or consume inputs
outside their package disable task caching. Build dependencies are declared in
the task graph rather than repeated inside test files.

The pre-commit hook checks formatting, lint, Cargo formatting, and Clippy. It does
not run the test suites; use the tasks above to verify behavior.
