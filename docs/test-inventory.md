# Tests and verification

Tasks are defined in each workspace's `vite.config.ts`. Run them with Vite+.

| Root task               | Scope                                                                                     |
| ----------------------- | ----------------------------------------------------------------------------------------- |
| `vp run check`          | Formatting, lint, TypeScript checks, Cargo formatting, and Clippy                         |
| `vp run test`           | Rust workspace tests and ordinary JS tests in packages, tools, Play, and Shape Lab        |
| `vp run fixtures:check` | Stillwind project outputs, Tailwind expectations, Webref data, and Tailwind ordering data |
| `vp run verify`         | All three tasks above                                                                     |
| `vp run verify:compat`  | Tailwind differential comparison, property ordering, numeric domains                      |
| `vp run verify:browser` | Generated CSS behavior in Chromium or Edge                                                |
| `vp run verify:wasm`    | WASM build and the binding contract shared with native tests                              |

`verify:browser` needs a Chromium-compatible executable. Set `STILLWIND_BROWSER`
when the default Edge path is unavailable. `verify:wasm` needs the
`wasm32-wasip1-threads` Rust target and its build toolchain. Neither environment is
required by ordinary tests. A requested environment's failure is reported by its
task rather than silently skipped.

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
