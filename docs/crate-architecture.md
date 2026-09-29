# Rust crate responsibility boundaries

This is the accepted crate structure for Stillwind. The Rust workspace has
twelve crates; there are no `core`, `tailwind`, or `grammar` compatibility crates.

The proposed framework descriptor placement and projection/editing boundaries
are documented in [framework projection design](framework-projection-design.md).

| Crate           | Owns                                                                                                                                |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `source`        | Source buffers, spans, coordinate conversion, projections, and content fingerprints                                                 |
| `diagnostics`   | Typed compilation errors and lint findings, message templates, reporting codes, labels, and lint suppression                        |
| `syntax`        | Utility expression tokenization and candidate syntax                                                                                |
| `css`           | CSS parsing primitives, value validation, math and color handling, custom properties, resource references, and output-template data |
| `semantic`      | Asset identities, finite values and reference constraints, module summaries, module identity, and versioned type facts              |
| `ir`            | Module IR, scan and finalized-analysis contracts, and frontend/provider interfaces                                                  |
| `js-oxc`        | Oxc parsing, macro extraction, scan/finalize implementation, and type-fact application                                              |
| `design-system` | Stillwind CSS directives, theme and registry construction, utility/variant evaluation, import expansion, and immutable snapshots    |
| `compiler`      | IR lowering, class/symbol allocation, source replacements, generated rules, CSS rendering, type declarations, and candidate queries |
| `project`       | Mutable analyses and artifacts, rule ownership, reloads, dependency indexes, producer selection, and coverage validation            |
| `napi`          | Node transport, serialization, and composition of the Rust services                                                                 |
| `cli`           | Filesystem/package resolution and command-line composition                                                                          |

`css` also exposes optional final asset minification and source-map composition through its
`minify` feature. See [CSS minification](css-minification.md) for the public API.

`crates/test-support` contains shared test sources, not a workspace crate.

## Dependency direction

The main production edges are:

```text
js-oxc -> ir -> semantic
          ^
project -> compiler -> design-system -> css
                                     -> syntax

All layers may use source.
diagnostics -> source
design-system, semantic, ir, js-oxc, compiler, project, napi, cli -> diagnostics
napi and cli compose the services they need.
```

`compiler` must not depend on `js-oxc` or `project`. `design-system` must not
depend on `compiler`. `css` must not depend on Stillwind's theme/registry model.
Compiler tests may use the Oxc frontend as a dev dependency.

`vp run crates:check` validates the workspace members and allowed
production/build dependencies using Cargo metadata. It also runs as part of
`vp run -r check`.

## Diagnostic contracts

`stillwind-diagnostics` owns two independent pairs: `CompileErrorKind` wrapped by
`CompileError`, and `LintKind` wrapped by `LintFinding`. Each kind uses `thiserror`
for display and an exhaustive `code() -> &'static str` implementation for reporting
identity. Different message templates have different variants. Multiple variants
may share a code, and a typed context may select different codes for one variant.
Lint codes are the rule names accepted by `swlint-*` directives.

Wrappers retain ordered labels and notes, plus an optional source name for compilation
error display. The first label is the primary location and controls suppression. Only lint findings can be suppressed. Detection creates
the appropriate kind directly: reporting codes do not decide whether an issue is
fatal. Candidate queries attach synthetic spans over the supplied candidate text.

The diagnostics crate depends only on `source` within the workspace. Producers return
shared compilation kinds directly and attach a `CompileError` once a source location
is available. Loader protocol errors remain beside the loader;
only protocol and external source/parser failures require boundary conversion.
Fact-provider failures carry a typed kind through `semantic` to module compilation. Oxc's open-ended parser reports use the fixed `JavaScriptParse` summary
and retain the external explanation in notes. CSS parser and projection validator
details are parameters of their respective, explicitly named message templates.

CLI and Node transports format kinds at the boundary and keep their existing
`code`/`rule`, `message`, location, and `notes` fields. Rust serialization preserves
the typed kind and wrapper metadata rather than a separately mutable message/code.
There are no diagnostic re-exports from `source`.

## Inputs and generated artifacts

`ModuleScan` and `ModuleAnalysis` are frontend-independent inputs in `ir`.
`OxcSourceAnalyzer` implements `SourceAnalyzer` in `js-oxc`. The backend consumes
the finalized analysis directly; it does not convert it back into an Oxc parse
result. A source-buffer convenience API accepts an explicit `SourceAnalyzer`.

`RuleKey`, `GeneratedRule`, `ModuleCssArtifact`, `CompiledModule`, and source
transform plans belong to `compiler`. They describe generated output rather
than frontend input. Project state owns module contributions to those outputs.

`semantic` remains distinct from `ir`: identities and finite cross-module facts
can be exchanged independently of the module's extracted style operations.

## Definition settings and generation settings

`DesignSystemOptions` contains definition-level property prefixes. A snapshot
resolves those defaults against CSS configuration and retains only the resolved
definition settings, theme, registries, source provenance, and fingerprints.

`CompilerOptions` contains host defaults plus class namespace, application
namespace, and generation policy. `design_system()` projects the loader inputs;
module compilers and renderers combine generation policy with the snapshot's
resolved prefixes. Different compilation settings can share one snapshot.

`StylesheetRenderer` owns CSS rendering and declaration generation.
`DesignSystemQuery` lives in `compiler` so queries reuse the real resolver and
emitter. Neither operation introduces a dependency from snapshots to codegen.

`StylesheetPlacement` is a compiler rendering input. `StylesheetSelection`,
`StylesheetCoverage`, and `StylesheetPlan` belong to `project`, which decides
which producers are available and whether a host's complete-coverage assertion
is valid.

## Cache and reload contracts

Artifact identity includes compiler build content, resolved compilation
settings, definition content, source/module identity, finalized analysis inputs,
frontend kind, adapter mode, and semantic revisions. Changing frontend IR for
the same source revision invalidates the artifact without giving the backend a
build dependency on the frontend implementation.

The compiler build fingerprint covers its implementation, production Rust
dependencies, CSS property data, manifests, and the lockfile. Authored builtin
CSS is an input owned by `packages/core/css`; Rust does not embed that package's
default stylesheets in production.

A session owns generation options independently of its snapshot. A reload
receives both the attempted snapshot and the next generation options and
publishes a coherent replacement. Failed watch reloads retain the previous
successful state. Build failures remain errors. Definition dependency indexes
are conservative; successful reloads still recompile retained analyses.

## Host boundary

`DesignSystemLoader` requests opaque files; the host resolves and reads them.
Import expansion preserves authored ordering, wrappers, physical provenance,
and resource owners. There is no general Rust `resolver` crate.

The JavaScript host and bundler adapters retain module resolution, environment
graphs, the TypeScript checker, asynchronous stale-load checks, and editor
document state. The Rust project layer owns artifacts and stylesheet plans,
not a duplicate bundler or TypeScript graph.

## Validation

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
vp run -r build
vp run -r typecheck
vp run -r test
vp run -r check
vp run fixtures:check
```

Framework acceptance commands and platform-specific source-map limits are in
[module graph implementation](module-graph-implementation.md).

### Refactor verification on 2026-09-10

- Rust workspace tests: 366 passed; the existing manual registry audit remains
  ignored. All 362 pre-existing Rust test functions were retained, with three
  regression tests added for generation settings, reloads, and changed frontend IR.
- Workspace Clippy with warnings denied, Rust formatting, builds, type checks,
  host/Vite tests, and package checks passed. All six review fixtures matched.
- Real browser acceptance passed for Vite, Next/Turbopack, Next/webpack, Rspack,
  Rsbuild, React Router, SolidStart, and both TanStack/Vite and TanStack/Rsbuild.
- Four source-map verifier tests cover project-relative webpack URLs, query
  hashes, file URLs, and rejection of a different source directory.

Rspack initially failed its verifier because `webpack:///static.js?hash` was
treated as a filesystem-relative name. The unchanged `3eeb5ef` native binding,
built separately and injected only into the baseline test process, reproduced
the failure. The verifier now resolves the URL against the project and checks
the exact original file, line, column, and source content. Both baseline and
refactored bindings passed the corrected probe; no compiler workaround was added.
