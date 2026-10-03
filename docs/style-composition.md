# Style composition v1

Status: implemented. Transport contracts are defined in [CSS emission](css-emission.md).

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

Across modules, the stylesheet linker closes selections over base producers and
establishes a stable topological order. Explicit CSS values include this closure.
The emission transports preserve the resulting cascade through managed document
updates, a complete stylesheet, or ordered manifest occurrences. See
[CSS emission](css-emission.md) and [CSS ordering](css-ordering.md).

Style dependency cycles and producer-module cycles are rejected. Unrelated
JavaScript import cycles remain valid. Independent builds and manually reordered
CSS need an application-level placement policy. Composition topology edits use
the same plan update mechanism as other edits; JavaScript changes use framework
HMR and may still require a reload.

## Acceptance gates

- Base/normal/hover example, multi-level and diamond composition.
- Returning to the same utility content does not reuse an ancestor class.
- Runtime conditional bases and conditional utilities retain evaluation behavior.
- No-delta composition, invalid bases, additional props, and nested calls.
- All possible dependencies, style cycles, and module-order cycles.
- Imported/re-exported producers and cache invalidation.
- Verified transport: development, production, reverse lazy-route order, shared
  bases, SSR/hydration where applicable, content HMR and dependency updates.

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

`classNamespace` must agree between server and client builds.

## Reported overlaps

Two lint rules report competing declarations in composed styles. Both
read the declarations of published rules, so they compare what a host delivers rather than
what the source spells, and both default to `warn`.

- `ambiguous-base-order`: one call composes two bases that compete for one declaration slot
  in the same variant context, and neither base is an ancestor of the other.
  Declaring the property in one base, or making one base derive from the other, removes the
  finding.
- `imported-base-interference`: a base owned by another module competes for a declaration
  slot with the call's own rules or its other bases.

One slot means the same property, or a shorthand and a longhand it covers (`margin` against
`margin-top`, including nested shorthands such as `border` against `border-top-width`).
That relation comes from the generated Webref table in the `css` crate, which is why the
rules need no hand-maintained property list. The same table carries Webref's logical property
groups, so a logical property also competes with the physical properties a writing mode can
map it onto: `margin-block-start` against `margin-top` and `margin-left`, `block-size`
against `width` and `height`. Distinct slots of one group stay independent, so
`margin-top` and `margin-bottom`, or `block-size` and `inline-size`, are never reported.

Findings name overlaps so authors can choose explicit relationships or remove
competing declarations. The comparison covers published artifacts only: a base compiled after
its derived module is compared once it is published, a request-scoped session that compiles
one module at a time only reports the overlaps inside that module, and document or authored
CSS never participates.

## Verification

The repeatable browser probe is `vp run test:e2e` inside
`e2e/css`.
It compares computed styles across development, single/split builds, SSR and Shadow DOM.
