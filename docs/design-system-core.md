# Using the compiler services

The implementation follows [the crate responsibility boundaries](./crate-architecture.md).
Loading, frontend analysis, compilation, and mutable project state have separate
crates. Hosts explicitly compose these services; there is no compatibility facade.

## Loading CSS

```rust
use stillwind_design_system::{DesignSystemLoader, DesignSystemLoadStep, DesignSystemOptions};

let mut loader = DesignSystemLoader::new(DesignSystemOptions::default());
let mut step = loader.provide(0, root_name, root_content);
let snapshot = loop {
    match step {
        DesignSystemLoadStep::NeedFile(requests) => {
            let request = requests.into_iter().next().expect("outstanding request");
            let id = request.id;
            // Host operation: resolve exactly (importer, authored specifier).
            let (name, content) = host_read(&request)?;
            step = loader.provide(id, name, content);
        }
        DesignSystemLoadStep::Complete(result) => break result?,
    }
};
```

`NeedFile` contains every outstanding request, sorted by monotonically increasing
IDs starting at 1. The first call supplies exactly one root file, conventionally
ID 0. `provide(id, name, content)` wraps `provide_many(files)`, where each
`CssFileInput` contains `id`, `name`, and `content`.

An async host may read the list concurrently and submit a batch, or supply any
subset as reads finish. Ready files are parsed immediately and can expose more
requests before earlier reads finish. Track in-flight IDs to avoid scheduling
repeated outstanding requests twice. Import expansion always follows authored
order. Duplicate or unknown response IDs are errors. Request ID exhaustion
returns terminal `Complete(Err(RequestIdExhausted))`; `u32::MAX` is never issued. An empty batch after the
root simply returns the current outstanding list.

Source names are opaque identities. A filesystem host should canonicalize the
root and imported names consistently. A resolver or read error belongs to the host: drop the
incomplete loader. Completion is terminal, including a failed completion.

All imports request buffers from the host, including package-owned definitions.
Import order, duplicate identities, cycles, conflicting contents,
layer placement, supports conditions, and media conditions are handled by the
loader. `snapshot.sources()` retains supplied names, edges and expansion
mappings; theme token spans point into their original source buffers.

`DesignSystemSnapshot::from_source` is a convenience for a single supplied
stylesheet with no external imports. If imports need host resolution, use the
loader loop instead.

## Analysis, compilation and queries

```rust
use stillwind_ir::SourceAnalyzer;
use stillwind_js_oxc::OxcSourceAnalyzer;
use stillwind_compiler::{
    ModuleCompiler, DesignSystemQuery, CompilerOptions, CompletionInput,
};

let scan = OxcSourceAnalyzer.scan(source); // Arc<SourceFile>
// Optional: pass scans to a TypeScriptFactsProvider before finalization.
let analysis = OxcSourceAnalyzer.finalize(scan, None);
let options = CompilerOptions::default();
let compiled = ModuleCompiler::new(snapshot.clone(), options.clone()).compile(&analysis);
let transformed = compiled.apply_to_source(&analysis.source)?;

let query = DesignSystemQuery::new(snapshot.clone(), options.clone());
let resolved = query.resolve_candidate("hover:bg-red-500");
let completions = query.complete_candidate(CompletionInput { prefix: "bg-".into() });
```

Scanning produces local IR, producer summaries, expression probes, source
diagnostics and lint directives. Finalization consumes optional versioned,
span-keyed `TypeFacts`; incompatible schemas, stale source revisions and invalid
byte spans produce integration diagnostics. The Rust frontend does not load or
traverse JavaScript dependencies. The host TypeScript service retains ownership
of checker snapshots and cross-module propagation. The native adapter implements
`TypeScriptFactsProvider` for the service's supplied facts and asset metadata.

The snapshot exposes resolved utility, variant, property and token registries,
declaration formats, relationship definitions, framework imports and lint policy.
Overloaded definitions remain grouped in immutable lookup views. Selected
built-in definitions are also retained by the snapshot.

Compilation and queries share candidate resolution and CSS emission. A compiled
module contains keyed rules, source transforms, classes, symbols, diagnostics,
lint findings and definition/asset dependencies. Rules retain their selector or
at-rule prelude and block body losslessly; the existing emitter is adapted at
the rule boundary without changing its serialization. Lint findings and policy
do not change CSS, transforms or generated names.

## Rendering a pinned snapshot

```rust
use stillwind_compiler::StylesheetRenderer;

let renderer = StylesheetRenderer::new(&snapshot, &options);
let document_css = renderer.render_base_stylesheet()?;
let module_css = renderer.render_module_stylesheet(&compiled.css.to_css())?;
let declarations = renderer.generate_vocabulary_types();
```

The snapshot owns definition settings. Rendering and class-generation policy
belong to the compiler. CSS-resolved prefixes override the loader defaults;
application namespace remains a separate compiler setting; each style call has its own identity.

## Sessions and reloads

```rust
use stillwind_project::{ProjectSession, SessionMode};

let mut session = ProjectSession::new(snapshot, SessionMode::Watch, options.clone());
let change = session.update_module(analysis)?;
let css = session.css()?;
session.remove_module("src/removed.tsx")?;

// Drive a fresh loader first; keep the active snapshot pinned during loading.
let change = session.reload(new_load_result, options)?;
```

Session mutations are exclusive actor operations; a host may put the session
behind a mutex. `snapshot()` returns an immutable clone for concurrent requests.
There is no watcher, I/O runtime or checker process inside the Rust services.

The stylesheet owns rule contributions and module membership. Removing a module
retains rules used by other modules. Serialization uses deterministic module
ordering, preserves the emitter's rule order and adds the pinned design system's
shared theme/base/property output once.

Successful CSS reloads recompile all retained analyses. If a watch reload cannot
produce a coherent successful set of artifacts, the previous set remains
active. Failed initial loads cannot construct a session. Build-mode failures are
returned to the host and do not retain successful module artifacts as fallback.
Source failures likewise retain the previous successful artifact only in watch
mode. Notifications distinguish changed CSS and changed transform plans.

Artifact keys include compiler content identity, resolved generation options,
snapshot content, finalized analysis inputs, module identity and source revision,
source file ID, frontend, adapter and optional semantic revision. Identical successful artifacts are reused. The compiler build
fingerprint includes compiler/dependency source, CSS property data, manifests and the lockfile, so
unpublished builds sharing a package version do not collide.

Definition fingerprints and dependency indexes are present. Definition dependency
sets are deliberately conservative; selective CSS-reload invalidation is not
enabled. A successful snapshot replacement currently invalidates all compiled
artifacts while retaining source analyses.

## Node and Vite

`@stillwind/native` exports `NativeDesignSystemLoader` and `NativeProjectSession`.
`loader.provide(id, name, content)` and
`loader.provideMany([{ id, name, content }, ...])` return JSON with
`kind: "needFile"` and `requests: [{ id, importer, specifier }, ...]`, or
`kind: "complete"` (and revision and source names). Load errors throw. Construct
a session from a completed loader, then call `compile`, `css`,
`remove` and `reload`. `loader.generateVocabularyTypes()` uses the same snapshot.

Vite resolves and reads outstanding requests concurrently, supplying each file
as soon as it is ready. Newly discovered imports start while earlier reads are
still pending; an ID is scheduled only once. After an error, late reads cannot
feed the stopped loader, and their rejections remain handled. Vite registers
supplied files for watching and uses the native session for CSS
contributions and symbol collision checks. CSS reload errors report a warning and keep the active development
output. Source graph traversal and the TypeScript checker stay in the host.
The CLI implements the same loader protocol with synchronous filesystem I/O.

## Verification commands

```powershell
$env:CARGO_INCREMENTAL='0'
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
vp run --filter @stillwind/native check
vp run --filter @stillwind/integration test
vp run fixtures:check
vp run tailwind:parity:all
```

Tests cover snapshot identity and fingerprints, imported source provenance,
loader failures, TypeScript fact freshness, lossless artifacts, query/compiler
agreement, shared rule ownership, cache reuse, concurrent compilation, lint
independence, native bindings and Vite reload behavior. Existing transform and
Tailwind fixture expectations are preserved.

Workspace builds and type checks cover the React example as well as the native,
host, and Vite packages. See [crate architecture](crate-architecture.md) for the
full validation commands and [module graph implementation](module-graph-implementation.md)
for framework acceptance and downstream source-map limitations.

### Fallible Rust boundaries

`SourceFile::new` returns `Result` and rejects buffers beyond the 32-bit source-coordinate limit. `Span::len` returns `None` for reversed spans, and `lex_utilities` returns a source error for invalid or overflowing token coordinates. Callers must propagate or handle these failures.

`StylesheetRenderer::render_stylesheet`, `ProjectStylesheet::module_css`/`serialize`, and `ProjectSession::css`/`update_module`/`remove_module` return `Result` for stylesheet failures. Compiler entrypoints that return diagnostic collections attach these failures as diagnostics. The Node interface returns successful JSON/string values and throws on boundary errors.
