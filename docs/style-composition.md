# Style composition v1

Status: implemented and verified within the transport boundaries listed below.

## Contract

`style(base, utilities)` lowers only the call to a small runtime merge of the
base props and the independently compiled delta props. Variable declarations,
JavaScript evaluation, and conditional selection remain ordinary JavaScript.
`style(a, b)` lowers to a merge without allocating a new CSS class.

Every rule-bearing style call has its own identity, regardless of whether a
consumer composes it. Content deduplication must not join distinct calls.
Conditional utility branches retain the existing compilation behavior and have
distinct branch identities within the call.

Bases must have statically enumerable style producers. All possible producers
contribute dependency edges, including dynamically selected alternatives. Bases
with additional props are rejected in v1; arbitrary props objects are not bases.
Direct nested style calls are rejected. Base argument order does not impose
precedence between unrelated bases. Composition results retain transitive base
dependencies even when they emit no rules.

## Ordering

Each base's rules precede its derived call's rules, preserving ordinary CSS
specificity, importance, and layers. This is not utility-string merging and is
not an unconditional derived-style override. A derived normal declaration does
not defeat a base hover declaration with higher specificity.

Sort style nodes topologically within a module. Reject style dependency cycles.
Project composition edges onto CSS producer modules and reject cycles in that
module graph, even when the style graph itself is acyclic. Do not reject unrelated
JavaScript import cycles. Do not build a global style interleaver or a final
chunk sorter.

Use the existing bundler dependency transport where actual CSS ordering is
verified. Reading dependencies before evaluating JavaScript is not by itself
proof of stylesheet order. Unsupported adapter/chunk configurations must be
documented explicitly; do not claim acceptance from compiler-only tests.

Dependency topology changes require a full reload. Ordinary content HMR is
allowed only where it preserves the required stylesheet order.

## Acceptance gates

- Base/normal/hover example, multi-level and diamond composition.
- Returning to the same utility content does not reuse an ancestor class.
- Runtime conditional bases and conditional utilities retain evaluation behavior.
- No-delta composition, invalid bases, additional props, and nested calls.
- All possible dependencies, style cycles, and module-order cycles.
- Imported/re-exported producers and cache invalidation.
- Verified transport: development, production, reverse lazy-route order, shared
  bases, SSR/hydration where applicable, content HMR and topology reload.

## Deliberate limits

No condition-exclusivity analysis, props inheritance, graph-aware content
deduplication, module interleaving, or incremental topology HMR. Unsupported
transport configurations will be recorded with the acceptance results.

## Implementation and compatibility

The public macro returns a branded `StyleResult`. Checker-only declaration
identities distinguish producers through imports, re-exports, conditional values,
assignments, and analyzable factories. These annotations never enter generated
JavaScript. Unknown producer sets are errors. Style results are intended to be
immutable; arbitrary mutation through opaque heap aliases is outside this
compile-time contract.

The compiler emits `stillwindMerge("className", base, delta)` (or `"class"` for
the corresponding framework). Interleaved runtime inputs use local call-expression
temporaries to preserve their evaluation order. Variable declarations and
conditional base expressions remain ordinary JavaScript. Native outputs expose
the validated `composition` edges for host invalidation and transport decisions.

`classIdentity` and content deduplication have been removed. `classNamespace`
remains available and must agree between server and client builds. Existing
generated class names and fixtures change as part of this migration.

## Verified transport and remaining boundaries

| Configuration                                                                     | v1 status                                                                                                                                                     |
| --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Single-module JS/TS composition                                                   | Compiler and runtime tests cover conditional bases/utilities, no-delta merges, evaluation order, invalid bases, and graph errors.                             |
| Vue, Svelte, Astro local composition                                              | Original-source mappings and framework compilation are tested. Cross-module checker projections remain unsupported.                                           |
| Vite JS/TS automatic CSS imports, default chunking                                | Browser-tested in development and production, including shared bases, multi-level and diamond composition, reverse lazy-route order, and React SSR/hydration. |
| Vite composition HMR                                                              | Base/content/topology edits use full reload. Ordinary HMR remains available for unrelated modules.                                                            |
| Webpack/Rspack/Rsbuild/Next loader                                                | Same-module composition only; cross-module composition is rejected pending adapter-specific ordering acceptance.                                              |
| Explicit module CSS values and standalone CSS projections                         | Same-module composition only; cross-module composition is rejected because their insertion order is caller-owned.                                             |
| Custom chunk grouping, independently built bundles, other framework SSR pipelines | Not covered by the Vite acceptance result; no ordering guarantee.                                                                                             |
| Native aggregate `session.css()` across multiple producers                        | Does not establish composition order. Use producer CSS and the dependency metadata with a verified host transport.                                            |

Development CSS uses short, immutable references so embedded source maps do not
exceed HTTP request limits. Production JS carriers retain their self-contained
payloads for independent/cached transforms. Automatic carrier imports follow the
producer's JavaScript dependencies; no final global CSS sorter is introduced.

The repeatable browser probe is `vp run verify:composition` inside
`experiments/module-graph`.
Its report is `experiments/module-graph/results/composition.json`.
