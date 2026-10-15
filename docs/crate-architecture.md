# Rust crate responsibility boundaries

The Rust workspace separates source analysis, CSS definitions, compilation, and
mutable project state.

The host projects Vue, Svelte, and Astro sources for Rust analysis. See
[SFC type checking](sfc-type-checking.md) for projection and coordinate contracts.

| Crate           | Owns                                                                                                                                |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `cli`           | Filesystem/package resolution and command-line composition                                                                          |
| `compiler`      | IR lowering, class/symbol allocation, source replacements, generated rules, CSS rendering, and candidate queries                    |
| `css`           | CSS parsing primitives, value validation, math and color handling, custom properties, resource references, and output-template data |
| `design-system` | Stillwind CSS directives, theme and registry construction, utility/variant evaluation, import expansion, and immutable snapshots    |
| `diagnostics`   | Typed compilation errors and lint findings, message templates, reporting codes, labels, and lint suppression                        |
| `ir`            | Module IR, scan and finalized-analysis contracts, and frontend/provider interfaces                                                  |
| `js-oxc`        | Oxc parsing, macro extraction, scan/finalize implementation, and type-fact application                                              |
| `napi`          | Node transport, serialization, and composition of the Rust services                                                                 |
| `project`       | Mutable analyses and artifacts, rule ownership, reloads, dependency indexes, producer selection, and coverage validation            |
| `semantic`      | Asset identities, finite values and reference constraints, module summaries, module identity, and versioned type facts              |
| `source`        | Source buffers, spans, coordinate conversion, projections, and content fingerprints                                                 |
| `syntax`        | Utility expression tokenization and candidate syntax                                                                                |
| `test-support`  | CSS fixtures, Markdown readers, Tailwind fixture input conversion, builtin package contents, and CSS token assertions               |

`css` also exposes optional final asset minification and source-map composition through its
`minify` feature. See [CSS minification](css-emission.md#final-asset-minification) for the public API.

Tests use `stillwind-test-support` through dev-dependencies. The crate reads the
standard CSS package from `packages/core`.

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

## Diagnostic contracts

`stillwind-diagnostics` owns compilation errors and lint diagnostics. `CompileDiagnostic`
owns each fatal message and its payload, and `CompileError` attaches source details.
`CompileErrorCode` corresponds to one diagnostic variant and exposes its unique
`SWnnmmm` code through `as_str()`. Internal consumers compare typed variants; external
reports serialize code strings. Each `CompileErrorClass` reserves a range of code
categories: `configuration` uses 01-09, `authoring` uses 10-29, `host` uses 30-49, and
`internal` uses 90-99. Reports include this class so a host can tell whether the user,
the host, or the compiler must change. The module documentation of `compilation::code`
lists the categories. Source failure context retains the operation and does not change
the diagnostic identity. `LintDiagnostic` owns the message template and concrete values
for a detected lint, and `LintFinding` attaches its source details. Different message
templates have different diagnostic variants.

`LintKind` identifies one lint rule and corresponds to one stable name accepted by project
configuration and `swlint-*` directives. `LintDiagnostic::kind()` selects that rule;
multiple diagnostic variants can select the same rule, and utility classification can
select different rules for one diagnostic variant. `LintConfig` has one setting variant
for each `LintKind`. `LintSettings` stores at most one setting for each rule and prevents
keys from disagreeing with their setting variants. Unconfigured rules do not enable
configured detection, but a producer can still detect them; reporting then uses `warn`.

Project configuration validates rule names and settings before loading resources.
Effective settings serialize to the host protocol as a map of rule names to severities.

Wrappers retain ordered labels and notes, plus an optional source name for compilation
error display. The first label is the primary location and controls suppression. Only
lint findings can be suppressed. Detection creates the appropriate diagnostic directly:
reporting codes do not decide whether an issue is fatal. Candidate queries attach
synthetic spans over the supplied candidate text.

The diagnostics crate depends only on `source` within the workspace. Producers return
shared compilation kinds directly and attach a `CompileError` once a source location
is available. Loader protocol errors remain beside the loader;
only protocol and external source/parser failures require boundary conversion.
Fact-provider failures carry a typed kind through `semantic` to module compilation.
Oxc's open-ended parser reports use the fixed `JavaScriptParse` summary and retain the
external explanation in notes. CSS parser and projection validator details are
parameters of their respective, explicitly named message templates.

CLI and Node transports format kinds at the boundary and keep their existing
`code`/`rule`, `message`, location, and `notes` fields. Rust serialization preserves
the typed kind and wrapper metadata rather than a separately mutable message/code.

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

`DesignSystemOptions` contains property prefixes and utility ordering. A snapshot
captures those host settings, theme definitions, registries, source provenance, and
fingerprints.

`CompilerOptions` contains host defaults plus class namespace, application
namespace, and generation policy. `design_system()` projects the loader inputs;
module compilers and renderers combine generation policy with the snapshot's
resolved prefixes. Different compilation settings can share one snapshot.

`StylesheetRenderer` owns CSS rendering.
`GeneratedRule::new` normalizes registration dependencies and fixes each rule's
syntax and placement. Its fields are read-only after construction; replacing a
rule requires constructing a new value. The constructor stores the identity of
its CSS, placement, and normalized registration dependencies. `content_key()`
returns that stored key.
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

## Service entrypoints

`DesignSystemLoader::provide` and `provide_many` accept host-supplied CSS files.
The first response supplies the root at ID 0. `NeedFile` contains outstanding
requests in ID order; the host resolves each importer and specifier. Responses
may arrive in any order or in partial batches. Ready files can expose additional
requests while earlier reads remain pending. Import expansion retains authored
order. Duplicate or unknown response IDs are errors. Completion is terminal,
including failed completion; a host resolution error ends that loading attempt.

`OxcSourceAnalyzer::scan` extracts macros, dependencies, and type probes.
`finalize` accepts optional versioned type facts in original source coordinates.
The host traverses dependencies and queries TypeScript before finalization.
`ModuleCompiler` consumes the finalized analysis. `DesignSystemQuery` pins editor
queries to a snapshot and uses the same candidate resolver as compilation.
Canonical queries prepare shared indexes lazily; `prepare_canonical_index` lets
a loader prepare them before publishing the snapshot.

`ProjectSession` owns exclusive updates, removal, and reload. Hosts can retain
immutable snapshot clones for concurrent reads. Successful CSS reloads recompile
retained analyses. Watch failures keep the last successful artifacts and report
the attempted revision's diagnostics; build failures remain errors.

Immutable source revisions and snapshots use shared ownership where they cross
threads. Naming callbacks use local ownership because they call JavaScript on
the owning thread. Immutable compact identities use `LeanStr`; growing output
buffers use `String`.

## Source identity and diagnostics

`SourceFile` owns immutable text, its scoped `FileId`, an optional absolute
`SourceUri`, an optional source-map source name, and coordinate tables. The URI
identifies an editable document; the source-map name is supplied independently.
Internal buffers without a URI cannot supply public diagnostic or edit locations.
One URI can have several revisions, identified by content fingerprints.

The host supplies separate identities for module resolution, compilation, checker
buffers, and SFC conversion. Rust does not derive diagnostic URIs from module IDs
or normalize opaque `ModuleId` values. A host rejects two resolver IDs assigned
to the same module ID before analysis. Resource owners refer to module IDs, which
the host maps back to resolver IDs.

Public locations contain the document URI, revision, a half-open UTF-8 byte range,
UTF-16 offsets, and zero-based line and UTF-16 column values. Generated and
projected findings are mapped back to the original document before publication.
Text diagnostics display one-based lines and columns.

`Reporter` resolves and validates locations, applies lint policy and suppression,
and produces a diagnostic batch. A batch cannot mix revisions of the same URI.
Each fix action contains at least one edit; overlapping edits are rejected.
Applying a fix requires every affected document's current fingerprint to match,
or the entire action is rejected.

## Node binding

`@stillwind/native` accepts typed objects and arrays. CSS, source text, and v3
source-map JSON remain strings. Session `compile` returns producer output and
unwrapped module CSS; `css` or `planStylesheet` assembles document CSS and reports
assembly errors. Standalone `compileSource` assembles CSS and returns assembly
failures as diagnostics with no generated code. `remove` returns whether the
producer was registered; `reload` reports project-level CSS changes.

The binding transports semantic candidates as `universe` or `finite` records.
Named exports use entry arrays so names such as `__proto__` survive conversion.
The native build generates `packages/native/binding.d.ts`; do not edit it by hand.
The generated ESM loader prefers the native addon and falls back to WASI.
`NAPI_RS_FORCE_WASI=error` requires WASI. Browser hosts need COOP and COEP headers
for shared WebAssembly memory.

See [verification](test-inventory.md) for build and test tasks and
[open work](TODO.md) for unresolved design and validation items.
