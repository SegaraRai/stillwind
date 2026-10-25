# Open work

This file contains unresolved design and validation work. Current supported
inputs and failure behavior are documented with their APIs. Each item below
identifies a remaining limitation and the evidence needed to resolve it.

## Conditional utility completions

Utility strings in `style()` conjunctions, such as `style(enabled && "opacity-50")`,
still suggest their own inferred strings. Evaluate suppression while preserving ref
template inputs and finite macro choices.

Sources: [style declaration](../packages/core/src/macro/style.ts),
[input checks](../packages/core/src/macro/input.ts).

## Playground editor state

The Playground reports `Cannot set null/undefined state` browser errors during static
acceptance. Trace the Monaco state update that receives the missing value. Confirm that
formatting, completion, and HMR complete with an empty browser error list.

Source: [static acceptance](../e2e/play/src/static.test.ts).

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

## Analysis work after source preparation

- Load dependencies found by the checker in batches before collecting type facts.
  Each entry must still resolve its own required imports and report missing inputs in its project.
- Reuse completed native analyses when entries share dependencies. Generate code, CSS,
  and full editor indexes only when a caller requests them. Lint and formatting must
  still validate assignments and declaration identities.
- Check cancellation between dependency batches and merge pending requests for the
  same revision. Later edits must not change editor answers that have already been returned.
- Keep `SourceFormatter` project state when its inputs have not changed. Reload it when
  configuration dependencies change. Changes outside open buffers currently reopen the workspace.

Use the existing project queues and prepared sources. These improvements should not
require a new graph system, persistent caches, extra workers, or a daemon.

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

## Remaining documentation capabilities

The utility explorer searches the default registry's finite completion catalog.
Add on-demand resolution for numeric and arbitrary candidates without enumerating
infinite domains. Show rule previews and required document support separately.
Exact CSS equivalence needs a public query contract that preserves selectors,
conditions, declaration order, and support requirements; property/value term
searches cannot prove that equivalence.

Macro references publish actual compiled code and generated CSS. A display of
each optimization pass requires compiler trace data; the current compiler has no
public pass-selection or trace API. Preserve the distinction between compiler
lowering and later bundler minification.

Compiler codes can change before the first release. Freeze published identities
or record redirects and retirement pages before tooling exposes permanent
links. Removed codes must not silently describe a different error.

Sources: [utility query API](../packages/native/binding.d.ts),
[canonical normalization](../crates/compiler/src/query/canonical/index.rs),
[runtime forms](runtime.md#lowering-and-materialization),
[compiler error codes](../crates/diagnostics/src/compilation/code.rs),
[authored reference pages](../apps/website/src/content/docs/docs/macros/style.mdx).

## Measurement coverage

`TypeScriptFactsService` enables checker timing collection but exposes only
process, snapshot, and filesystem counters. Expose round trips, transferred bytes,
and server time in host debug telemetry so checker work can be distinguished
from transport and host work.

Source: [checker service](../packages/host/src/semantics/node/service.ts).

Synthetic build/watch benchmarks and reused checker workloads do not establish
latency for a typed application through filesystem events, subsequent module
requests, browser CSS application, and framework HMR. Define and measure that
complete path before assigning application latency guarantees.

Sources: [architecture benchmarks](../benchmarks/research/architecture.bench.ts),
[host benchmarks](../benchmarks/compiler/host.bench.ts).

Browser WASM preparation of the standard CSS registry can exceed its 500ms hard
budget. Profile CSS loading and canonical index preparation, then reduce their
cost while preserving preparation once per revision. Recheck the preparation
median and uncached query percentiles with `vp run bench:compiler:browser`.

Sources: [browser measurement](../benchmarks/compiler/canonical.browser.test.ts),
[operation budgets](../benchmarks/performance/DEVELOPMENT.md#budget-policy).

## Registry context preparation

Creating the three [registry context](css-emission.md#registry-contexts) indexes took
about 2.5ms after a 32ms preparation of the package CSS in a native release build.
Measure the browser cost with `vp run bench:compiler:browser`.

Sources: [registry statements](../crates/design-system/src/registry/contexts.rs),
[theme index](../crates/design-system/src/theme/index.rs).

## Skipped CSS statements

Stylesheet lint reports [skipped statements](css-emission.md#skipped-statements) only
for the sources that a host marks for linting. Bundler integrations warn about every
skipped statement, but other hosts do not report a broken import or directive in package
CSS, such as a file under `node_modules`. Decide whether every host shows these records
as load warnings.

An import whose specifier the host cannot resolve has no file to watch, so creating the
file later does not reload the graph until another watched file changes.

The renderer still fails for a malformed `@stillwind-require-properties`, a requirement
without a `@stillwind-property` registration, and invalid `--theme()` syntax. Evaluate
skipping them with a lint finding as loading does.

Keyframe frames, view transition definitions, and `@apply` inside a utility body do not
check runtime conditions for each candidate. When their declarations cannot be lowered,
`invalid-runtime-condition` reports them at the whole module instead of the candidate.

Sources: [skipped statements](../crates/design-system/src/skipped.rs),
[stylesheet lint](../crates/compiler/src/compiler/stylesheet/lint/skipped.rs).

## Lit server rendering

The element directive applies props in the browser. Add an SSR path that emits the
same classes and inline values before adding Lit server acceptance.

Source: [Lit directive](../packages/core/src/frameworks/lit.ts).
