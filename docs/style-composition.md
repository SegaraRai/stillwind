# Style composition v1

Status: implemented and verified within the transport boundaries listed below.

## Contract

`style(base, utilities)` lowers only the call to a small runtime merge of the
base props and the independently compiled delta props. Variable declarations,
JavaScript evaluation, and conditional selection remain ordinary JavaScript:
each user expression is emitted once and evaluated once per call, in source
order. `style(a, b)` lowers to a merge without allocating a new CSS class.

Every rule-bearing style call has its own identity, regardless of whether a
consumer composes it. Content deduplication must not join distinct calls.
Conditional utilities keep their source structure and have distinct class
identities within the call, at any nesting depth. See
[structure-preserving style lowering](style-fragment-lowering.md) for the
fragment runtime, the emission tiers, and the ordering contract.

Bases must have statically enumerable style producers. All possible producers
contribute dependency edges, including dynamically selected alternatives. Bases
with additional props are rejected in v1; arbitrary props objects are not bases.
Direct nested style calls are rejected. Base argument order does not impose
precedence between unrelated bases. Composition results retain transitive base
dependencies even when they emit no rules.

## Ordering

Composition constrains only CSS placement: within a module, each base's rules
precede its derived call's rules. Ordinary CSS specificity, importance, and
layers still decide the cascade before source order. A derived normal declaration
does not defeat a base hover declaration with higher specificity.

Across modules, base-before-derived placement depends on the host transport.
Custom chunk grouping, independently concatenated bundles, aggregate stylesheet
APIs, unverified SSR pipelines, and later CSS reordering do not guarantee it.
Explicit module CSS values and standalone projections follow the caller's
insertion or import order and require the base producer's CSS separately.

[CSS ordering](css-ordering.md) describes the four levels - declaration, class
unit, module unit, and delivered CSS - that this contract sits in.

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

The compiler emits either a plain props object (no runtime class expression), a
single `stillwindClsx(...)` class property (runtime class expressions that no
other user expression separates), or `stillwindMerge(...)` with fragments in
source order. Composition bases are converted with `stillwindToFragment`. The
runtime merge materializes the framework class and style formats and never
leaves a symbol key in the result. Variable declarations and conditional base
expressions remain ordinary JavaScript. Native outputs expose the validated
`composition` edges for host invalidation and transport decisions.

`classIdentity` and content deduplication have been removed. `classNamespace`
remains available and must agree between server and client builds. Existing
generated class names and fixtures change as part of this migration.

## Verified transport and remaining boundaries

| Configuration                                                                     | v1 status                                                                                                                                                                                                                             |
| --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Single-module JS/TS composition                                                   | Compiler and runtime tests cover conditional bases/utilities, no-delta merges, evaluation order, invalid bases, and graph errors.                                                                                                     |
| Vue, Svelte, Astro local composition                                              | Original-source mappings and framework compilation are tested. Cross-module checker projections remain unsupported.                                                                                                                   |
| Vite JS/TS automatic CSS imports, default chunking                                | Browser-tested in development and production, including shared bases, multi-level and diamond composition, reverse lazy-route order, and React SSR/hydration.                                                                         |
| Vite composition HMR                                                              | Base/content/topology edits use full reload. Ordinary HMR remains available for unrelated modules.                                                                                                                                    |
| Webpack/Rspack/Rsbuild/Next loader                                                | Cross-module composition is accepted. The bundler's module graph delivers the order, and a later duplicate of a base's payload ranks after the derived rules; overlaps are reported by the lint rules below.                          |
| Explicit module CSS values and standalone CSS projections                         | Cross-module composition is accepted. A value or projection carries only its own module's rules, so the caller's insertion or import order decides the cascade and a composing producer needs its base's value or projection as well. |
| Custom chunk grouping, independently built bundles, other framework SSR pipelines | Not covered by the Vite acceptance result; no ordering guarantee.                                                                                                                                                                     |
| Native aggregate `session.css()` across multiple producers                        | Does not establish composition order. Use producer CSS and the dependency metadata with a verified host transport.                                                                                                                    |

Development CSS uses short, immutable references so embedded source maps do not
exceed HTTP request limits. Production JS carriers retain their self-contained
payloads for independent/cached transforms. Automatic carrier imports follow the
producer's JavaScript dependencies; no final global CSS sorter is introduced.

## Reported overlaps

Two lint rules report the calls whose cascade the composition contract leaves open. Both
read the declarations of published rules, so they compare what a host delivers rather than
what the source spells, and both default to `warn`.

- `ambiguous-base-order`: one call composes two bases that compete for one declaration slot
  in the same variant context, and neither base is an ancestor of the other. The order between
  unrelated bases is not part of the contract, so which declaration wins is undefined.
  Declaring the property in one base, or making one base derive from the other, removes the
  finding.
- `imported-base-interference`: a base owned by another module competes for a declaration
  slot with the call's own rules or its other bases. Another module's rules reach the element
  in host-transport order, which is verified for Vite's default chunking only.

One slot means the same property, or a shorthand and a longhand it covers (`margin` against
`margin-top`, including nested shorthands such as `border` against `border-top-width`).
That relation comes from the generated Webref table in the `css` crate, which is why the
rules need no hand-maintained property list. The same table carries Webref's logical property
groups, so a logical property also competes with the physical properties a writing mode can
map it onto: `margin-block-start` against `margin-top` and `margin-left`, `block-size`
against `width` and `height`. Distinct slots of one group stay independent, so
`margin-top` and `margin-bottom`, or `block-size` and `inline-size`, are never reported.

Both rules are advisory. A host with a verified order still decides the overlap; it decides
it in a way the source does not state, which is why the finding names the property instead
of failing the build. The comparison covers published artifacts only: a base compiled after
its derived module is compared once it is published, a request-scoped session that compiles
one module at a time only reports the overlaps inside that module, and document or authored
CSS never participates.

## Verification

The repeatable browser probe is `vp run verify:composition` inside
`experiments/module-graph`.
Its report is `experiments/module-graph/results/composition.json`.
