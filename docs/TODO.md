# Open work

This file contains unresolved design and validation work. Current supported
inputs and failure behavior are documented with their APIs. Each item below
identifies a remaining limitation and the evidence needed to resolve it.

## Editor snapshot ownership

`EditorSnapshot` retains a mutable `ModuleAnalysis`. `AnalysisProject` exposes it
through `AnalysisResult.graph`, and `BrowserProject` appends reports after creating
the editor snapshot. Readonly fields do not prevent mutation through other aliases.
Define the publication boundary for sources, dependencies, reports, and sorting
configuration. Verify that retained hover, fixes, sorting, and reports remain
consistent after graph mutation and later project updates. An editor failure from
this mutation has not been reproduced.

Sources: [snapshot](../packages/host/src/editor/snapshot.ts),
[analysis project](../packages/host/src/project/analysis.ts),
[browser project](../packages/host/src/browser/index.ts).

## Semantic fingerprints and prefix rewriting

Semantic revisions hash `Debug` representations of facts and asset metadata in
several entrypoints. Define one deterministic representation shared by those
entrypoints. Confirm invalidation when facts or asset support change without
making incidental debug formatting part of the cache contract.

Sources: [project input](../crates/napi/src/api/project.rs),
[session facts](../crates/napi/src/session/facts.rs),
[compiler API](../crates/compiler/src/compiler/api.rs).

`DesignSystemOptions::rewrite_generated` and `CompilerOptions::rewrite_generated`
duplicate literal prefix substitution. Consolidate the responsibility and define
how generated identifiers stay separate from authored strings. Preserve swapped
and overlapping configured prefixes and resource references.

Sources: [design-system options](../crates/design-system/src/options.rs),
[compiler options](../crates/compiler/src/options.rs).

## Rendering rejection reports

`StylesheetRenderer::render_compilation` drains rejected host-generated names.
Carrier and project stylesheet rendering do not return the same rejection report.
Expose rendering rejections consistently and test development fallback through
each public rendering path.

Source: [renderer](../crates/compiler/src/renderer.rs).

## Selective CSS reloads

Definition dependencies remain conservative, and successful CSS reloads recompile
retained analyses. Selective invalidation needs dependencies for successful
resolution, failed lookup, new definitions, and namespace resets. Verify added,
changed, and removed definitions before using narrower reload selections.

Sources: [module compiler](../crates/compiler/src/module_compiler.rs),
[project session](../crates/project/src/session/state.rs).

## Assets from compiled packages

Assets whose declaration source is absent from the graph have no supported
versioned package manifest channel. Define identity records, value-shape versions,
naming revisions, CSS ownership, and collision reservations before accepting them.
Keep checker propagation separate from the delivery of actual asset definitions.

Sources: [module graph](../packages/host/src/compilation/module-graph.ts),
[semantic values](../crates/semantic/src/symbolic.rs).

## View-transition keys

The linter counts transition-name suppliers but does not detect repeated-render
or unstable-key use. Establish which framework facts can support actionable
warnings without treating valid cross-component pairing as a collision.

Source: [view-transition analysis](../crates/compiler/src/style_analysis/view_transition.rs).

## Public generated-rule invariants

Public generated-rule fields can separate the saved key from current content.
Review validation at public input boundaries and decide whether mutation should
be restricted or followed by explicit revalidation. Test caller-constructed
values rather than assuming they came from compiler constructors.

Source: [generated rules](../crates/compiler/src/module_css.rs).

## Measurement coverage

`TypeScriptFactsService` enables checker timing collection but exposes only
process, snapshot, and filesystem counters. Expose round trips, transferred bytes,
and server time in host debug telemetry so checker work can be distinguished
from transport and host work.

Source: [checker service](../packages/host/src/semantics/type-facts.ts).

Synthetic build/watch benchmarks and reused checker workloads do not establish
latency for a typed application through filesystem events, subsequent module
requests, browser CSS application, and framework HMR. Define and measure that
complete path before assigning application latency guarantees.

Sources: [architecture benchmarks](../benchmarks/research/architecture.bench.ts),
[host benchmarks](../benchmarks/compiler/host.bench.ts).
