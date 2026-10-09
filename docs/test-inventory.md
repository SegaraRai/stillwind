# Tests and verification

Tasks are defined in each workspace's `vite.config.ts`. Run them with Vite+.

| Root task                       | Scope                                                                                         |
| ------------------------------- | --------------------------------------------------------------------------------------------- |
| `vp run check:generated`        | Tailwind fixture expectations and catalogs, Webref data, and both Tailwind ordering outputs   |
| `vp run check`                  | Formatting, lint, TypeScript checks, Cargo formatting, Clippy, and generated-data consistency |
| `vp run generate`               | Rust input data, project expectations and viewer catalog, Tailwind catalogs and fixtures      |
| `vp run test:browser`           | CSS cascade, rendered filters, and ref scopes in Vitest Browser Mode                          |
| `vp run test:e2e`               | Framework and bundler browser acceptance                                                      |
| `vp run test:fixtures:projects` | Generated project JS, CSS, diagnostics, source maps, and the viewer catalog                   |
| `vp run test:performance`       | Performance budgets; `test:performance:full` uses the complete workload                       |
| `vp run test:tailwind`          | Tailwind differential comparison, property ordering, and numeric domains                      |
| `vp run test:wasm`              | WASM build, loader contracts, package fallback, production bundling, and Browser Mode         |
| `vp run test`                   | Rust tests, ordinary JS tests, and project output comparisons                                 |
| `vp run verify`                 | `check` and `test`                                                                            |

The root `test.projects` configuration selects ordinary JS projects.
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
  Browser behavior uses the dedicated browser task.
- Project fixtures under `fixtures/projects/` record generated JS, CSS,
  diagnostics, and source mappings. Their expected files and the fixtures viewer
  catalog are produced by the same runner.

Tests protect current observable behavior. A test's reason to exist is the
contract it verifies, rather than the refactor or audit that first exposed it.
Configuration inventories and lists of forbidden former implementations are not
runtime contracts.

## Tool responsibilities

| Package                                     | Responsibility                                                                              | Stillwind prerequisites |
| ------------------------------------------- | ------------------------------------------------------------------------------------------- | ----------------------- |
| `@stillwind-internal/tools-fixture-css`     | Load CSS for verification consumers                                                         | Core CSS source files   |
| `@stillwind-internal/tools-projects`        | Generate or compare project expectations and the viewer catalog                             | Host and native builds  |
| `@stillwind-internal/tools-tailwind-compat` | Compare native output against Tailwind, run browser checks, and report coverage             | Native binding          |
| `@stillwind-internal/tools-tailwind`        | Generate upstream expectations, registrations, and ordering data; compare declaration order | None                    |
| `@stillwind-internal/tools-webref`          | Generate and check Rust Webref data                                                         | None                    |
| `@stillwind-internal/tools-workspace`       | Share task cache configuration                                                              | None                    |

Tailwind comparison adapters live beside the comparison that uses them. The CSS
loader used by integration tests is available as `@stillwind-internal/tools-fixture-css/stillwind-css`.
Declaration-order comparison has no dependency on the native compiler; compiling
fixture candidates belongs to its separate fixture verifier.

Run a focused tool task with, for example,
`vp run @stillwind-internal/tools-tailwind-compat#test:tailwind:parity` or
`vp run @stillwind-internal/tools-tailwind-compat#test:tailwind:property-order`.
`analyze:tailwind:inventory` writes each coverage report under a unique
`target/tailwind-inventory/run-*` directory. `--output` selects the parent directory.
The command prints the selected run directory. `analyze:tailwind` prints a
difference analysis. These reports are explicit operations rather than ordinary
test prerequisites.

## Updating generated files

| Task                                                                         | Updates                                             |
| ---------------------------------------------------------------------------- | --------------------------------------------------- |
| `vp run generate:fixtures:projects`                                          | Stillwind project expected files and viewer catalog |
| `vp run generate:fixtures:tailwind`                                          | Tailwind fixture expectations                       |
| `vp run generate:tailwind-order`                                             | Rust Tailwind property order                        |
| `vp run generate:webref`                                                     | Rust Webref property data                           |
| `vp run @stillwind-internal/tools-tailwind#generate:fixtures:tailwind-order` | Utility-order fixture                               |
| `vp run @stillwind-internal/tools-tailwind#generate:tailwind-registrations`  | Registration catalogs and candidate lists           |
| `vp run @stillwind/native#generate:platform-packages`                        | Platform package manifests and runtime dependencies |

Use `test:*` for behavior and expected-output assertions, `check:*` for static
consistency, and `verify` for an aggregate that includes both. `generate:*`
produces outputs from local inputs, including regeneration. `refresh:*` is
reserved for fetching mutable external inputs. `fmt` writes formatted files;
`fmt:check` and `fmt:rust:check` compare formatting without writing.

Rust-data and fixture generators in `tools/webref`, `tools/tailwind`, and
`tools/projects` write their outputs by default. Pass `--check` to compare without
updating their expected files.
`utility-order.ts` generates the Rust property table; `utility-order-fixtures.ts`
generates the ordering fixture used by Rust tests. Native builds depend on the
Rust generator. The fixture generator runs only when explicitly selected or
through `generate`. `test:rust` reads the ordering fixture; it includes that JSON
in its cache inputs even when Cargo reuses compiled tests.

Webref and the Tailwind property table are Rust compilation inputs. Their
generators run before native and WASM compilation and before `test:rust`.
`lint:rust` depends on their consistency checks, so stale tables fail Clippy's
prerequisites instead of being rewritten by that task.

Platform package generation runs when the configured targets or napi-rs CLI's
runtime dependencies change. It creates package metadata and has no compilation
prerequisite. It is selected separately from `generate`. TanStack fixture packages
also define `generate:routes`; their development, build, and typecheck tasks
depend on that route generation.

Build dependencies and route generation are declared in the task graph. Vite
configurations defer loading built integration plugins until the build or server
starts, so task discovery works before any build. `vp i` installs JavaScript
dependencies; native builds also require Rust and the host linker. WASM tasks
require the WASI Rust target and toolchain, and browser tasks require Chrome.

`tools/workspace` owns the private `@stillwind-internal/tools-workspace` package. It exports
TypeScript sources directly and has no runtime dependencies. Consumers declare
it as a development dependency and import `@stillwind-internal/tools-workspace/task-cache`.

Finite deterministic tasks cache their inputs and outputs automatically. The
shared configuration adds Rust sources that Cargo may skip reading, tracks
result-affecting environment variables, and excludes disposable fixture directories.
Test tasks restore no temporary files. Fixture generators restore their generated
files. Native compilation caches `.build/native`, and a short uncached task installs
changed bindings into `dist` and `binding.d.ts`. This keeps cache restoration away
from addons that Windows processes may have loaded. WASM compilation caches
`dist/wasm`; both builds exclude Cargo's incremental cache. When retaining
integration test workspaces with `STILLWIND_KEEP_FRAMEWORK_FIXTURES`, pass
`--no-cache` to force execution and retain new evidence, for example:

```powershell
$env:STILLWIND_KEEP_FRAMEWORK_FIXTURES = '1'
vp run --no-cache @stillwind/integration#test:frameworks
```

Vite+ options precede the task name. Arguments after the task name are passed to
the task's command.

Servers, browser acceptance, performance measurements, formatting writes, native
binding installation, and artifact collection do not cache results.
Tailwind registration tasks depend on `prepare:tailwind-source`. It downloads
four files from the commit pinned in `tools/tailwind/upstream-source.json`, checks
their hashes, and writes them under `tools/tailwind/.cache/upstream/<SHA>`.
It deletes newly downloaded files if acquisition or verification fails. Preparation
reads the installed `tailwindcss/package.json` and creates the source JSON when
it is missing or its version differs. The acquisition code defines the required
files independently of that JSON. It resolves the installed release's tag to a
full commit SHA and checks the downloaded package version before saving the JSON.
Existing source files only undergo verification. The task uses automatic input
and output tracking, including reads of cached source files. Registration tasks
track those files as inputs and validate their hashes and installed package
version. Their deterministic results cache.
`generate:fixtures:tailwind` generates candidates before their expected CSS through
its registration dependency.
`collect:release` requires artifacts for every
configured target; `collect:local` builds the host native and WASM targets first.

Fixture verification compares tracked files. Native builds and Rust tests
generate compiler input data before compiling. Tailwind preparation can update
`upstream-source.json` as a prerequisite of generated-data checks when the
installed version changes; fixture comparison commands leave their expected
files unchanged. `generate` also updates fixture
expectations and the viewer catalog. The task graph places compiler input
generation before native compilation, which precedes project-fixture generation.
Run graph-dependent tasks through `vp run`;
`--parallel` bypasses dependency ordering. Separate invocations that generate and
compare the same files still need to run sequentially.

The pre-commit hook checks formatting, lint, Cargo formatting, and Clippy. It does
not run the test suites; use the tasks above to verify behavior.

## Review fixtures

Projects under `fixtures/projects` have a stable `project.json`, a theme, ordered
inputs, and generated expected JS, CSS, diagnostics, and maps. Keep broad Tailwind
fixtures and package-local test inputs in their owning harnesses. Add the smallest
representative project when output changes, regenerate explicitly, review the
expected diff, then compare with `vp run test:fixtures:projects`.

`vp run --filter @stillwind-internal/apps-fixtures-viewer dev` serves the catalog.
The viewer's build and development tasks generate it first. Production output is
static. Module fragments omit shared theme/base/property layers; `project.css`
contains the complete stylesheet. Formatted `.min.css` siblings show optimization
changes rather than compact production byte sizes.

Viewer links use `/#/projects/<project-id>/files/<percent-encoded-source-path>`.
Project IDs and source paths remain stable so review links keep working. Use the
relevant permalink in a PR or issue when a representative transform changes.

Performance workloads and measurement boundaries are documented beside their
[benchmarks](../benchmarks/README.md). Open design and validation work belongs in
[TODO.md](TODO.md).
