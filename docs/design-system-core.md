# Using the design-system core

The implementation of [the accepted architecture](./design-system-architecture.md)
is exported by `stillwind-core`. New integrations should use these domain APIs;
`Compiler` and `CompileResult` remain compatibility facades for existing callers
and fixtures, and delegate to the same snapshot and module compilation pipeline.

## Loading CSS

```rust
use stillwind_core::{DesignSystemLoader, DesignSystemLoadStep, DesignSystemOptions};

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
order. Duplicate or unknown response IDs are errors. An empty batch after the
root simply returns the current outstanding list.

Source names are opaque identities. A filesystem host should canonicalize the
root and imported names consistently. A resolver or read error belongs to the host: drop the
incomplete loader. Completion is terminal, including a failed completion.

Framework imports select vendored definitions. Other imports request buffers
from the host. Import order, duplicate identities, cycles, conflicting contents,
layer placement, supports conditions, and media conditions are handled by the
loader. `snapshot.sources()` retains supplied names, edges and expansion
mappings; theme token spans point into their original source buffers.

`DesignSystemSnapshot::from_source` is a convenience for a single supplied
stylesheet with no external imports. If imports need host resolution, use the
loader loop instead.

## Analysis, compilation and queries

```rust
use stillwind_core::{
    SourceAnalyzer, OxcSourceAnalyzer, ModuleCompiler, DesignSystemQuery,
    CompletionInput,
};

let scan = OxcSourceAnalyzer.scan(source); // Arc<SourceFile>
// Optional: pass scans to a TypeScriptFactsProvider before finalization.
let analysis = OxcSourceAnalyzer.finalize(scan, None);
let compiled = ModuleCompiler::new(snapshot.clone()).compile(&analysis);
let transformed = compiled.apply_to_source(&analysis.source)?;

let query = DesignSystemQuery::new(snapshot.clone());
let resolved = query.resolve_candidate("hover:bg-red-500");
let completions = query.complete_candidate(CompletionInput { prefix: "bg-".into() });
```

Scanning produces local IR, producer summaries, expression probes, source
diagnostics and lint directives. Finalization consumes optional versioned,
span-keyed `TypeFacts`; incompatible schemas, stale source revisions and invalid
byte spans produce integration diagnostics. The Rust frontend does not load or
traverse JavaScript dependencies. The Vite TypeScript service retains ownership
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

## Sessions and reloads

```rust
use stillwind_core::{ProjectSession, SessionMode};

let mut session = ProjectSession::new(snapshot, SessionMode::Watch);
let change = session.update_module(analysis);
let css = session.css();
session.remove_module("src/removed.tsx");

// Drive a fresh loader first; keep the active snapshot pinned during loading.
let change = session.reload(new_load_result)?;
```

Session mutations are exclusive actor operations; a host may put the session
behind a mutex. `snapshot()` returns an immutable clone for concurrent requests.
There is no watcher, I/O runtime or checker process inside the core.

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

Artifact keys include compiler content identity, snapshot content fingerprint,
module identity and source revision, source file ID, frontend, adapter and optional
semantic revision. Identical successful artifacts are reused. The compiler build
fingerprint includes compiler source, CSS definitions and the lockfile, so
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
`remove` and `reload`. `loader.generateReactTypes()` uses the same snapshot.

Vite resolves and reads each request batch concurrently through its resolver,
registers supplied files for watching, and uses the native session for CSS
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
vp run --filter @stillwind/vite test
vp run fixtures:check
vp run tailwind:parity:all
```

Tests cover snapshot identity and fingerprints, imported source provenance,
loader failures, TypeScript fact freshness, lossless artifacts, query/compiler
agreement, shared rule ownership, cache reuse, concurrent compilation, lint
independence, native bindings and Vite reload behavior. Existing transform and
Tailwind fixture expectations are preserved.

The React example production build also passes. Its additional `tsc --noEmit`
check currently reports `TS2590` in `packages/react/src/index.ts:19` because the
utility/variant union is too large. This predates the refactor: a detached build
of `cad1f5f` produces an identical example vocabulary file (SHA-256
`858865bf08894313f3c6183e99a18921a9b6654bd167dc47772645484b86b7f7`), and the React
type definition and TypeScript configuration are unchanged. Native and Vite
package type checks pass.
