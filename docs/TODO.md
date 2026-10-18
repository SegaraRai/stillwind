# Open work

This file contains unresolved design and validation work. Current supported
inputs and failure behavior are documented with their APIs. Each item below
identifies a remaining limitation and the evidence needed to resolve it.

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

## Removing utility definitions

`@utility` definitions of one name add to each other, as in Tailwind, so a stylesheet
cannot remove a built-in utility or a plugin's utility. Add an explicit directive, such
as `@stillwind-undef-utility <name>`, when a project needs it. Define whether it removes
earlier definitions only or every definition of the name, and how it interacts with
`(functional)` plugin definitions and the `duplicate-definition` lint.

Sources: [utility matching](../crates/design-system/src/evaluation/utilities/matching.rs),
[CSS emission](css-emission.md#utility-definitions-with-the-same-name).
