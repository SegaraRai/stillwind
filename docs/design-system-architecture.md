# Stillwind design system architecture

Status: accepted design for the next compiler refactor.

This document fixes the public responsibility boundaries for loading Stillwind
CSS configuration, analyzing source modules, compiling analyzed modules, editor
queries, and HMR. It is intended to be sufficient context for implementing the
refactor in a new work session.

## Decisions

1. Parsed project CSS is represented by an immutable, cheaply cloneable
   `DesignSystemSnapshot`.
2. `DesignSystemLoader` is a caller-driven state machine. It never reads files,
   resolves paths, performs network I/O, or chooses sync versus async I/O.
3. The first stylesheet and every subsequently requested stylesheet enter the
   loader through `provide(id, name, content)` or `provide_many(files)`.
4. A source name is opaque to the compiler. The loader may compare, clone, and
   report it, but must never interpret it as a filesystem path or URL.
5. Source analysis is single-module and design-system-independent. It does not
   construct or traverse a JavaScript module graph.
6. Cross-module and non-local value propagation is an optional TypeScript
   integration. Rust does not implement a second module or data-flow engine.
7. `ModuleCompiler` and `DesignSystemQuery` use the same internal candidate
   parser and resolver.
8. HMR state and generated-artifact caches belong to `ProjectSession`, not to
   the immutable design system.
9. Reloading CSS constructs a new snapshot. It never mutates a snapshot already
   in use.
10. A failed development reload keeps the last-known-good snapshot active and
    reports the new diagnostics. An initial load or non-watch build has no such
    fallback.

## Rust module layout

Directory-backed modules use `module/mod.rs`, rather than a sibling `module.rs`.
Keep `lib.rs` and `mod.rs` at 200 lines or fewer, aiming for 100 where a natural
responsibility split permits it. Other handwritten Rust files stay at 800 lines
or fewer, preferably 400 where they can be divided naturally. Generated files
are exempt. Module entry points collect shared types and exports; substantial
implementation and test support belong in focused sibling files.

## Architecture

```text
host configuration
      +
provide(0, root_name, root_content)
      │
      ▼
DesignSystemLoader state machine
      │
      ├─ NeedFile [requests] ──► host resolves and reads concurrently or serially
      │                         host calls provide / provide_many
      │
      └─ Complete(Result<DesignSystemSnapshot, LoadError>)

                   Arc<DesignSystemSnapshot>
                       │              │
                       │              └─────────────────────┐
                       ▼                                    ▼
SourceAnalyzer ──► ModuleAnalysis ──► ModuleCompiler   DesignSystemQuery
                                          │             candidate CSS,
                                          ▼             completion, hover
                                    CompiledModule
                                          │
                                          ▼
                                   ProjectStylesheet

ProjectSession owns the current snapshot, module analyses, compiled artifacts,
dependency indexes, and HMR invalidation. The four domain components above do
not own a watcher or bundler lifecycle.
```

## 1. Design system snapshot

### Name

Use `DesignSystemSnapshot` for the concrete immutable value. APIs may use the
shorter `DesignSystem` type alias if it improves ergonomics, but documentation
must retain the snapshot terminology because replacement semantics are
important.

`Engine` is not the preferred name. The value contains resolved styling
semantics; it does not execute source analysis, own a TypeScript process, or
manage HMR.

### Ownership and cloning

```rust
#[derive(Clone)]
pub struct DesignSystemSnapshot {
    inner: Arc<DesignSystemData>,
}
```

Cloning a snapshot clones an `Arc`; it does not duplicate registries or source
text. `DesignSystemData` is immutable after construction. A compile or editor
request obtains one clone at its start and uses that same revision for the
entire request.

### State

```rust
struct DesignSystemData {
    revision: DesignSystemRevision,
    sources: CssSourceGraph,
    config: ResolvedConfig,

    theme: ThemeRegistry,
    utilities: UtilityRegistry,
    variants: VariantRegistry,
    properties: PropertyRegistry,
    declaration_formats: DeclarationFormatRegistry,
    relationships: RelationshipRegistry,

    imports: FrameworkImports,
    layers: LayerConfiguration,
    candidate_order: CandidateOrder,
    lint_policy: LintPolicy,

    diagnostics: Vec<DesignSystemDiagnostic>,
    fingerprints: DefinitionFingerprints,
}
```

The current `ThemeIndex` is the migration source for most of these fields. The
new registries are resolved lookup structures, not unrelated public `Vec`
fields.

Built-in definitions may be parsed once into a process-wide immutable
`Arc<BuiltinDefinitions>`. A snapshot contains the resolved view produced by
combining the selected built-ins, framework imports, theme CSS, and authored
definitions. Callers must not have to consult separate global registries to
understand the snapshot's behavior.

The snapshot does not contain:

- JavaScript or TypeScript source modules;
- `ModuleAnalysis` or generated module CSS;
- a TypeScript checker process or checker objects;
- Vite module IDs, watchers, or dev-server handles;
- mutable HMR caches.

### Identity and fingerprints

`DesignSystemRevision` identifies one complete successful load. It may be a
monotonic process-local value. Cache correctness must additionally use stable
content fingerprints; a monotonic revision alone is not reusable across
processes.

`DefinitionFingerprints` records stable hashes for independently invalidatable
semantic groups and definitions:

```rust
struct DefinitionFingerprints {
    config: Fingerprint,
    ordering: Fingerprint,
    lint_policy: Fingerprint,
    theme_tokens: BTreeMap<ThemeTokenId, Fingerprint>,
    utilities: BTreeMap<UtilityDefinitionId, Fingerprint>,
    variants: BTreeMap<VariantDefinitionId, Fingerprint>,
    properties: BTreeMap<PropertyDefinitionId, Fingerprint>,
}
```

The first implementation may invalidate all compiled modules on every snapshot
change. These fingerprints must still be part of the model so selective
invalidation does not require another public redesign.

## 2. Design system loader

### Responsibility

`DesignSystemLoader` parses and links a CSS source graph into one snapshot. It
is a deterministic state machine driven by source buffers supplied by its
caller.

It owns:

- partially parsed CSS sources;
- internal source IDs and provenance;
- pending import work;
- cycle and duplicate detection;
- accumulated load diagnostics;
- the construction state for the final registries.

It does not own:

- a filesystem abstraction;
- path, URL, package, or import-map resolution;
- synchronous or asynchronous I/O;
- file watching;
- retry policy;
- last-known-good snapshot selection.

It also has no shutdown or rollback work. Dropping an incomplete loader is the
complete cancellation mechanism.

### Opaque source names

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CssSourceName(Arc<str>);
```

The Rust representation may be a string-like token for FFI convenience, but
its contents are opaque. The loader may use it as a map key, attach it to
diagnostics, and return it as an importer. It must not join it with a specifier,
normalize separators, inspect extensions, canonicalize it, or assume that it
names a local file.

The host is solely responsible for interpreting:

```text
(importer: CssSourceName, specifier: authored CSS string)
```

and choosing the `CssSourceName` supplied with the next `provide` call.

### Public state machine

The initial source is supplied through the same `provide` / `provide_many` API
as imported sources. Construction alone does not name or load an entrypoint.

```rust
pub struct DesignSystemLoader {
    // private state
}

pub enum DesignSystemLoadStep {
    NeedFile(Vec<CssFileRequest>),
    Complete(Result<DesignSystemSnapshot, DesignSystemLoadError>),
}

pub struct CssFileRequest {
    pub id: u32,
    pub importer: CssSourceName,
    pub specifier: String,
    pub kind: CssDependencyKind,
}

pub enum CssDependencyKind {
    Import,
}

impl DesignSystemLoader {
    pub fn new(options: DesignSystemOptions) -> Self;

    pub fn provide(
        &mut self,
        id: u32,
        name: CssSourceName,
        content: Arc<str>,
    ) -> DesignSystemLoadStep;

    pub fn provide_many(
        &mut self,
        files: impl IntoIterator<Item = CssFileInput>,
    ) -> DesignSystemLoadStep;
}

pub struct CssFileInput {
    pub id: u32,
    pub name: CssSourceName,
    pub content: Arc<str>,
}
```

If the host cannot satisfy a `NeedFile` request, it reports its own resolution
or I/O error and drops the loader. External failures are not fed back into the
state machine. Calling `provide` when no source is expected is a loader protocol
error.

`NeedFile` returns all outstanding requests, ordered by ID. The loader issues
IDs starting at 1 and increasing monotonically. The initial `provide_many` must
contain exactly one root file, conventionally ID 0. `provide` is a single-file
wrapper around `provide_many`; subsequent responses must match outstanding IDs.
Unknown or duplicate response IDs are protocol errors. Attempting to allocate
`u32::MAX` terminates with `Complete(Err(RequestIdExhausted))` rather than
wrapping the counter or returning a partially allocated request list.

Hosts may supply any subset in any order as reads finish, or read a whole batch
in parallel and call `provide_many` once. A partially supplied batch does not
block parsing: each supplied source is parsed immediately and its imports become
requests in the next step. CSS expansion preserves authored import order,
independent of response order. Hosts scheduling reads incrementally must track
in-flight IDs because unresolved requests appear again in later steps.

### State transitions

```text
AwaitingInitial
  └─ provide(0, root_name, root_content)
       ├─ NeedFile([requests])
       │    └─ host resolves and reads any subset by request ID
       │         └─ provide(id, name, content) / provide_many(files)
       │              └─ NeedFile([remaining + newly discovered requests])
       │                   or Complete(...)
       └─ Complete(Ok(snapshot)) / Complete(Err(load_error))
```

Completion is terminal. The completed loader is not reused for HMR. A reload
starts a fresh loader and produces a fresh snapshot. Dropping the loader from
any incomplete state is safe and requires no compiler cleanup.

`Complete(Err(...))` is reserved for errors the loader can determine from
provided responses, such as invalid request IDs, invalid CSS, invalid framework
directives, import cycles, conflicting source identities, or registry-linking
failures. A missing
or unreadable requested source remains a host error.

### Sync and async hosts

Synchronous host:

```rust
let mut step = loader.provide(0, root_name, root_content);
loop {
    match step {
        DesignSystemLoadStep::NeedFile(requests) => {
            let request = requests.into_iter().next().expect("outstanding request");
            let id = request.id;
            let (name, content) = read_sync(&request)
                .map_err(|error| HostLoadError::new(request, error))?;
            step = loader.provide(id, name, content);
        }
        DesignSystemLoadStep::Complete(result) => break result,
    }
}
```

An asynchronous host may resolve and read the request list concurrently, then
call `provide_many`, or call `provide` as individual reads finish. The synchronous
loop above needs no queue of its own. No async runtime enters the Rust core.

### Imports, cycles, and duplicate sources

- The loader parses authored import specifiers but never resolves them.
- The host-selected `CssSourceName` is the identity used for cycle and duplicate
  detection.
- Imports of an already provided name reuse the earlier parsed source.
- A cycle is diagnosed with the chain of opaque names and authored specifiers.
- The source graph retained by the snapshot exposes every supplied name so the
  host can register watches or cache dependencies.
- Conflicting content supplied for an already known name is a protocol error;
  one load operation represents one coherent source snapshot.

## 3. Source analyzer

### Name and boundary

Use `SourceAnalyzer` for the frontend and `ModuleAnalysis` for its finalized
output. The current `ParsedJavaScript`, `ModuleIr`, and `ModuleSummary` are the
main migration inputs.

Source analysis is single-module. It recognizes import bindings inside that
module, but does not resolve or load imported modules and does not build a Rust
module graph.

### Two-phase TypeScript integration

Analysis has an optional two-phase form so TypeScript can receive producer and
probe information before finalized non-local facts are available:

```rust
pub trait SourceAnalyzer {
    fn scan(&self, source: Arc<SourceFile>) -> ModuleScan;

    fn finalize(
        &self,
        scan: ModuleScan,
        facts: Option<&TypeFacts>,
    ) -> ModuleAnalysis;
}
```

`ModuleScan` contains local extraction plus the declarations and source spans
needed to construct TypeScript shadow inputs. `ModuleAnalysis` contains the
language-neutral IR consumed by compilation.

```rust
pub struct ModuleScan {
    pub module_id: ModuleId,
    pub source: Arc<SourceFile>,
    pub source_revision: SourceRevision,
    pub local_ir: ModuleIr,
    pub summary: ModuleSummary,
    pub type_probes: Vec<TypeProbe>,
    pub lint_directives: Vec<LintDirective>,
    pub diagnostics: Vec<SourceDiagnostic>,
}

pub struct ModuleAnalysis {
    pub module_id: ModuleId,
    pub source: Arc<SourceFile>,
    pub source_revision: SourceRevision,
    pub ir: ModuleIr,
    pub summary: ModuleSummary,
    pub lint_directives: Vec<LintDirective>,
    pub diagnostics: Vec<SourceDiagnostic>,
    pub semantic_revision: Option<SemanticRevision>,
}
```

Without TypeScript facts, `finalize` supports only the intentionally limited
local/direct cases. Cross-module values and non-trivial conditional propagation
are unsupported rather than approximated by a Rust data-flow implementation.

### Optional TypeScript service

The optional project-level integration is named `TypeScriptFactsProvider` at
the Rust boundary. A Vite or LSP host may back it with the existing long-lived
TypeScript 7 sidecar.

```rust
pub trait TypeScriptFactsProvider {
    fn update(&mut self, scans: &[ModuleScan]) -> TypeFactsUpdate;
}
```

This provider, not `SourceAnalyzer`, owns dependency traversal, TypeScript
snapshots, invalidation, and checker process lifecycle. Checker-native objects
never enter `ModuleAnalysis`; they are immediately converted into versioned,
span-keyed `TypeFacts`.

## 4. Module compiler

### Name and API

Use `ModuleCompiler`, because this phase emits both CSS and source transform
plans. `CssGenerator` would understate its responsibility.

```rust
#[derive(Clone)]
pub struct ModuleCompiler {
    design_system: DesignSystemSnapshot,
}

impl ModuleCompiler {
    pub fn compile(&self, module: &ModuleAnalysis) -> CompiledModule;
}
```

The compiler is immutable and cheaply cloneable. Per-compilation scratch state
is local to `compile`, permitting parallel module compilation against one
snapshot.

### Output

```rust
pub struct CompiledModule {
    pub key: ModuleArtifactKey,
    pub code_transforms: Vec<SourceTransform>,
    pub css: ModuleCssArtifact,
    pub classes: Vec<GeneratedClass>,
    pub symbols: Vec<GeneratedSymbol>,
    pub diagnostics: Vec<CompileDiagnostic>,
    pub lint_findings: Vec<LintFinding>,
    pub dependencies: CompilationDependencies,
}
```

`ModuleCssArtifact` contains structured generated rules before serialization.
String-only module CSS is insufficient for safe removal, reference counting,
stable ordering, source maps, and HMR deduplication.

```rust
pub struct ModuleCssArtifact {
    pub rules: Vec<GeneratedRule>,
}
```

Every compiled module reports the design-system definitions and external assets
that affected it:

```rust
pub struct CompilationDependencies {
    pub candidates: BTreeSet<CandidateId>,
    pub theme_tokens: BTreeSet<ThemeTokenId>,
    pub utilities: BTreeSet<UtilityDefinitionId>,
    pub variants: BTreeSet<VariantDefinitionId>,
    pub properties: BTreeSet<PropertyDefinitionId>,
    pub assets: BTreeSet<AssetIdentity>,
}
```

Lint findings do not change `code_transforms`, CSS rules, or generated names.
Unrecoverable compile diagnostics prevent the affected artifact from replacing
its last successful value.

## 5. Design system query API

### Name and purpose

Use `DesignSystemQuery` for editor, LSP, fixture-review, REPL, and debugging
queries against one snapshot.

```rust
#[derive(Clone)]
pub struct DesignSystemQuery {
    design_system: DesignSystemSnapshot,
}

impl DesignSystemQuery {
    pub fn resolve_candidate(&self, candidate: &str) -> CandidateResolution;
    pub fn complete_candidate(&self, input: CompletionInput) -> Vec<CompletionItem>;
    pub fn hover_candidate(&self, candidate: &str) -> Option<CandidateHover>;
    pub fn diagnose_candidate(&self, candidate: &str) -> Vec<LintFinding>;
}
```

`CandidateResolution` is structured and includes the snapshot revision:

```rust
pub struct CandidateResolution {
    pub revision: DesignSystemRevision,
    pub parsed: Option<ParsedCandidate>,
    pub generated_rules: Vec<GeneratedRule>,
    pub required_properties: Vec<PropertyDefinitionId>,
    pub referenced_tokens: Vec<ThemeTokenId>,
    pub diagnostics: Vec<CandidateDiagnostic>,
}
```

`ModuleCompiler` and `DesignSystemQuery` must delegate to one internal
`CandidateResolver`. Completion must enumerate the same registries and grammar
that compilation uses. It must not maintain a second utility implementation.

An LSP request pins one `DesignSystemSnapshot` clone at the beginning of the
request. Results report its revision so stale asynchronous responses can be
discarded by the host.

## 6. Project session and HMR

### Responsibility

`ProjectSession` is the mutable orchestration layer used by Vite, CLI watch,
and an LSP workspace. It is not part of the immutable design-system semantics.

```rust
pub struct ProjectSession {
    current: ArcSwap<DesignSystemSnapshot>,
    analyses: HashMap<ModuleId, ModuleAnalysis>,
    artifacts: HashMap<ModuleId, CompiledModule>,
    stylesheet: ProjectStylesheet,
    dependencies: DependencyIndex,
}
```

Bindings that cannot expose `ArcSwap` may use a lock or actor while preserving
the same atomic snapshot replacement semantics.

### CSS reload

```text
watched CSS source changes
    │
    ▼
host creates a fresh DesignSystemLoader
    │
    ▼
host drives NeedFile/provide until Complete
    │
    ├─ success
    │    ├─ compare old/new fingerprints
    │    ├─ atomically publish new snapshot
    │    ├─ invalidate affected CompiledModule entries
    │    ├─ rebuild affected ProjectStylesheet rules
    │    └─ invalidate the virtual CSS module
    │
    └─ failure
         ├─ report diagnostics for the attempted revision
         └─ retain the last-known-good snapshot and artifacts in watch mode
```

The old snapshot remains alive through outstanding `Arc` clones. No request may
observe registries from two revisions.

For the first implementation, every successful design-system replacement
invalidates all compiled artifacts. `ModuleAnalysis` remains reusable because
it does not depend on the design system. Later, `DefinitionFingerprints` and
`CompilationDependencies` permit selective recompilation.

Some configuration changes affect transformed source as well as CSS. Prefix,
generated naming, runtime marker, or adapter-codegen changes therefore
invalidate both outputs. Theme-value-only changes normally invalidate module
CSS and the project stylesheet, but not source analysis.

### Source reload

```text
source module changes
    │
    ├─ invalidate its ModuleScan and ModuleAnalysis
    ├─ update optional TypeScriptFactsProvider
    ├─ invalidate modules whose TypeScript facts changed
    ├─ compile affected analyses against one pinned design-system revision
    ├─ remove old CSS contributions
    ├─ add new CSS contributions
    └─ notify the host only for outputs whose content changed
```

### Project stylesheet

`ProjectStylesheet` owns structured rule contributions and stable output order.
It does not concatenate module CSS strings.

```rust
pub struct ProjectStylesheet {
    rules: BTreeMap<RuleKey, RuleEntry>,
}

struct RuleEntry {
    rule: GeneratedRule,
    users: BTreeSet<ModuleId>,
}
```

Removing one module removes its membership from each rule. A shared rule remains
while any module uses it. Serialization adds design-system base/theme/property
output and generated module rules in the canonical layer and candidate order.

### Invalidation matrix

| Change                        | Invalidate                                                  |
| ----------------------------- | ----------------------------------------------------------- |
| Source text                   | module scan, analysis, artifact; dependent TypeScript facts |
| TypeScript facts              | affected module analyses and artifacts                      |
| Theme token value             | dependent compiled CSS and project stylesheet               |
| Utility or variant definition | dependent compiled CSS and project stylesheet               |
| Property definition           | affected compiled CSS and project stylesheet                |
| Layer or candidate order      | entire project stylesheet                                   |
| Prefix or generated naming    | compiled CSS and transformed source                         |
| Lint policy                   | lint results only                                           |
| CSS load failure during HMR   | attempted-revision diagnostics; retain active outputs       |

Every cache key includes at least the compiler version, design-system content
fingerprint, module source revision, frontend kind, adapter mode, and optional
TypeScript semantic revision.

## 7. Diagnostics and failure boundaries

The boundaries remain typed:

- `DesignSystemLoadError`: the snapshot cannot be constructed;
- `SourceDiagnostic`: source syntax or supported-expression extraction failure;
- `CompileDiagnostic`: structured source cannot be compiled under the pinned
  design system;
- `LintFinding`: advisory policy result that never changes compilation output;
- `IntegrationDiagnostic`: TypeScript provider, host protocol, or transport
  failure.

A loader protocol misuse is distinct from invalid user CSS. A host resolution
or read failure is likewise a host/integration error, not a
`DesignSystemLoadError`; the host can attach the request's importer and authored
specifier to its own diagnostic before dropping the loader.

## 8. Mapping from the current implementation

| Current concept                | Target concept                                        |
| ------------------------------ | ----------------------------------------------------- |
| `ThemeIndex`                   | input to `DesignSystemData` registries                |
| `parse_tailwind_theme`         | internal parsing step of `DesignSystemLoader`         |
| zero-sized `Compiler`          | split into loader, analyzer, compiler, and query APIs |
| `ParsedJavaScript`             | split into `ModuleScan` and `ModuleAnalysis`          |
| `ModuleIr`                     | retained as language-neutral analyzed source IR       |
| `ModuleSummary`                | retained as TypeScript shadow/manifest input          |
| `CompileResult`                | replaced by structured `CompiledModule`               |
| Tailwind `OnceLock` registries | shared `BuiltinDefinitions`, resolved into snapshots  |
| Vite `cssByModule`             | `ProjectStylesheet` rule contributions                |
| Vite `symbolsByModule`         | `CompiledModule.symbols` in `ProjectSession`          |
| Vite `TypeScriptFactsService`  | implementation of `TypeScriptFactsProvider`           |

## 9. Implementation sequence

1. Introduce `DesignSystemSnapshot` around the existing parsed theme and
   built-in registries without changing generated output.
2. Implement `DesignSystemLoader` as the specified state machine and route the
   existing single-file entrypoint through the initial `provide` call.
3. Add imported-source requests and source-graph provenance. Keep resolution in
   each host binding.
4. Split current Oxc output into `ModuleScan` and `ModuleAnalysis`; retain
   current local behavior.
5. Adapt the existing TS7 service to the `TypeScriptFactsProvider` boundary.
6. Extract `ModuleCompiler` and structured `CompiledModule` without changing
   CSS or transformed-code snapshots.
7. Extract the shared `CandidateResolver`, then add `DesignSystemQuery`.
8. Replace Vite's string maps with `ProjectSession` and
   `ProjectStylesheet` contributions.
9. Start with full artifact invalidation on snapshot replacement, then enable
   selective invalidation using recorded fingerprints and dependencies.

Every step must preserve exact Tailwind-compatibility fixtures and existing
source-transform fixtures. Architectural extraction is not permission to alter
CSS semantics.

## Non-goals of this refactor

- Implementing filesystem or package resolution in Rust core.
- Making the loader itself async.
- Adding Rust cross-module JavaScript analysis.
- Storing TypeScript checker objects in Rust snapshots.
- Making lint affect generated code or CSS.
- Requiring Vite to use Rust-specific path semantics.
- Optimizing selective invalidation before the full-invalidation path is
  correct.
