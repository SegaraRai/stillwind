# CSS ordering

Status: implemented. This document is the single source of truth for how
generated Stillwind CSS is ordered; the other documents link here instead of
restating the rules. Examples use short, illustrative class names (`._a`,
`.sw_x`); compiled output carries hashed names.

## Principles

- Stylesheet order and selector specificity decide the cascade. The order of
  class names in the `class` attribute never does, so Stillwind fixes rule
  order at compile time instead of relying on how a component spells classes.
- Cascade resolution runs `layer` -> `importance` -> `specificity` -> `order`.
  Ordering only decides ties between declarations that already agree on layer,
  importance, and specificity.
- Generated order must be deterministic: the same source produces the same CSS
  order in development, production, server, and client builds, independent of
  module arrival order.
- Stillwind adds no global sorter. Every host already owns an order it
  preserves (the module import graph, document order), and composition follows
  that order instead of replacing it.
- Tailwind CSS v4 compatibility is a requirement rather than an aspiration: at
  L2 the built-in surface reproduces Tailwind's order exactly, and at L1 it is
  effectively compatible, so only orderings a user can observe must match.

## The four levels

| Level                     | Decides                               | Governed by                                                                                      |
| ------------------------- | ------------------------------------- | ------------------------------------------------------------------------------------------------ |
| L1 declaration order      | declarations inside one rule          | built-in utility definition order; user definitions keep their source position                   |
| L2 block order            | selector blocks inside one class unit | plain block first, then variant blocks in the canonical variant order, then outside-nested rules |
| L3 unit order             | class units inside one module         | composition topology, then source order; inside a call, the base unit before conditional units   |
| L4 module and chunk order | modules in the delivered CSS          | the host's import and concatenation order                                                        |

The levels nest: L1 applies inside a block, L2 inside a class unit, L3 inside a
module, and L4 across modules. All four sit inside the `utilities` layer; the
layer statement itself is authored in document CSS.

## Tailwind CSS v4 compatibility

Ordering compatibility with Tailwind CSS v4 is a requirement for L1 and L2, not
a best effort. A Tailwind project must be able to migrate to Stillwind without
any declaration changing which one wins. That is a requirement about the
observable result, not about the text of the stylesheet, and it covers the
built-in surface; authored CSS keeps whatever order its author wrote.

The two levels meet the requirement differently:

- L2 matches Tailwind exactly. The built-in variant definitions are placed in
  Tailwind's variant order, so the emitted block order is the same.
- L1 matches Tailwind effectively. Every ordering a user can observe is the
  same, and a difference that cannot be observed is allowed (the pairs are
  listed under L1).

L3 and L4 are Stillwind contracts. Composition and host transport have no
Tailwind counterpart, so they are specified here instead of matched.

## L1 - declaration order inside one rule

Rule:

- The built-in surface is effectively compatible with Tailwind CSS v4's
  declaration order: every pair of declarations that can affect each other
  keeps Tailwind's relative order, and pairs that cannot affect each other may
  differ.
- A built-in utility takes its position from its `@stillwind-utility`
  definition. Built-in definitions share one global source order; when they are
  split across files, the registry manifest's import order decides the
  cross-file order. Arbitrary-property definitions take positions in the same
  sequence.
- A user `@utility` definition keeps its source position. Built-ins are
  reordered only among the slots they already occupy, so a user definition never
  moves.
- Contested shorthand and longhand declarations keep their order; a later
  declaration only replaces an earlier one when the two cannot interact.
- Equal positions fall back to the candidate spelling, which keeps the output
  stable.

Why: migrating from Tailwind should not require auditing which declaration
wins, so the built-in surface is required to be effectively compatible with
Tailwind - the same observable result, not the same textual order - and the
definitions are positioned with that goal in mind. Using the definition
position as the sort key keeps the requirement inside the CSS itself: there is
no second ordering table that can drift from the definitions, and a reviewer
can see where a utility sits relative to the ones it can affect.
Grouping the definitions by feature domain keeps the registry readable; the
price is that properties which cannot affect each other may sit in a different
relative order than in Tailwind, which is unobservable and therefore accepted.

Which pairs must match Tailwind's order:

- the same property written twice,
- a shorthand and any of its longhands, recursively and including reset
  longhands, such as `margin` and `margin-top`,
- a logical and a physical spelling of the property in the same logical group,
  such as `margin-left` and `margin-inline-start`,
- shorthands that share a longhand, such as `text-wrap` and `white-space`
  (both write `text-wrap-mode`), and
- any declaration against `all`, except custom properties.

Which pairs may differ: declarations that cannot affect each other, such as
`color` and `margin`. Their relative order is unobservable, so the registry
keeps its feature-domain layout instead of copying Tailwind's file order.

Example - both spellings produce the same declaration order for built-ins:

```tsx
style("p-4 m-2");
style("m-2 p-4");
```

```css
.sw_x {
  margin: calc(var(--spacing) * 2);
  padding: calc(var(--spacing) * 4);
}
```

Two user-defined utilities keep the order they were written in, because their
positions are their source positions.

## L2 - block order inside one class unit

Rule:

- A class unit may own several selector blocks. The plain block is emitted
  first, then the variant blocks in the canonical variant order, then the rules
  that exist only inside an at-rule from a CSS-defined utility.
- The canonical variant order is the order of the built-in `@stillwind-variant`
  definitions - for example `hover` before `disabled` - so it does not depend
  on how the utility string was spelled.
- The canonical order reproduces Tailwind CSS v4's variant order; that
  compatibility is a requirement rather than a preference.

Why: at equal specificity the later block wins, so a canonical variant order is
what makes state precedence deterministic: the state that follows another in
the canonical order is the one that wins, whatever order the utilities were
written in. Reproducing Tailwind's variant order makes that precedence
portable - a project that migrates keeps the same result between states, and
authors who already know Tailwind's order can rely on it here. Placing the
definitions in Tailwind's order, instead of mapping a Stillwind-specific order
onto it, keeps the two from drifting apart as variants are added.

Example - both spellings emit the same block order:

```tsx
style("hover:text-red-500 disabled:text-blue-500");
style("disabled:text-blue-500 hover:text-red-500");
```

```css
.sw_x:hover {
  color: var(--color-red-500);
}
.sw_x:disabled {
  color: var(--color-blue-500);
}
```

A disabled element that is also hovered therefore shows the disabled color,
whichever spelling the author used.

Boundary: L2 applies inside one class unit. Two separate calls follow L3, so the
canonical variant order does not reach across them.

## L3 - unit order inside one module

Rule - a module emits in this order:

1. The definitions the module references (the compiler's `definitions` band). Inside one
   view-transition definition, the group rule carrying the unconditional options
   precedes the rule of each conditioned option, in written order.
2. Style calls in composition order: every base's rules precede the derived
   call's rules, and calls without a dependency relationship keep source order.
   Composition cycles are rejected rather than resolved, because no order can
   satisfy them.
3. Inside one call: the unconditional base unit first, then the conditional
   units in source order of their literals. Unconditional utilities form one
   base unit whatever argument position they were written in; nested conditions
   use pre-order (outer before inner, consequent before alternate).
   A view transition follows the same rule: the class property is one
   declaration, so the unit that owns the element class list references one
   `var(--slot,)` per guarded use and each guarded unit fills its own slot. The
   resolved list is the union of the slots the element declares, so two
   conditions that hold together contribute both tokens instead of replacing
   one another.

Why: "a conditional delta overrides the base" and "a derived style composes
over its base" must not depend on the class attribute or on argument order. A
conditional unit always owns its own class unit and is never merged into the
base unit.

Example - composition sets the band order, conditions order the units inside a
band:

```tsx
const base = style("p-2", a && "p-4");
const props = style(base, "m-2", a && "m-4");
```

```css
/* base call, unconditional unit */
._a {
  padding: calc(var(--spacing) * 2);
}
/* base call, conditional unit */
._b {
  padding: calc(var(--spacing) * 4);
}
/* derived call, unconditional unit */
._c {
  margin: calc(var(--spacing) * 2);
}
/* derived call, conditional unit */
._d {
  margin: calc(var(--spacing) * 4);
}
```

Nested conditions keep their source pre-order:

```tsx
style("p-2", a && (b ? "p-4" : "m-2"));
```

```css
/* p-2, the base unit */
._a {
  padding: calc(var(--spacing) * 2);
}
/* a && b, the true branch */
._b {
  padding: calc(var(--spacing) * 4);
}
/* a && !b, the false branch */
._c {
  margin: calc(var(--spacing) * 2);
}
```

Tooling note: each class unit records the conditional tests that select it,
outermost to innermost with polarity, so hosts can invalidate and analyze
conditional style without re-reading the source expression.

## L4 - module and chunk order

Rule: each compiled module carries its CSS through a real CSS import that
follows that module's JavaScript dependencies (its carrier). Bundlers
concatenate CSS in import-graph order, so a default Vite production build keeps
a base module's CSS ahead of a derived module's CSS even when everything lands
in one file.

Why: no final global CSS sorter exists. Order comes from the graph the host
already preserves, and composition is validated against that graph (a cycle
that would require an impossible order is an error).

Verified today: development and production Vite with default chunking, shared
bases, multi-level and diamond composition, lazy routes loaded in reverse
order, and React SSR/hydration. Base, content, and topology edits use a full
reload; ordinary HMR stays available for unrelated modules.

Not guaranteed:

- custom chunk grouping that changes how CSS is concatenated,
- independently built bundles that an outer host concatenates,
- aggregate APIs that return one stylesheet assembled from several producers,
- other framework SSR pipelines, and
- a later pass that reorders the emitted CSS.

Beyond that transport, every path that composes across modules delivers whatever
order its host decides: the loader follows the bundler's module graph, an explicit
module CSS value carries only its own module's rules, and a standalone projection
carries only its producer's. A delivered order can therefore put a base's rules
after one of its derived modules' rules, which is why an overlap the contract
leaves open is reported by the `ambiguous-base-order` and
`imported-base-interference` lints instead of being ordered; see
[style composition](style-composition.md).

## Reasoning about a conflict

Resolve the cascade first, then apply the level that is still open:

1. Different layers? The layer statement decides.
2. `!important`? Importance decides.
3. Different specificity? The more specific selector wins, whatever the order.
   Variant selectors add specificity, so `._x:hover` beats `._y`.
4. Equal specificity? Order decides: L3 across units (composition, then source
   order), L2 inside one unit (canonical variant order), and L1 inside one rule.

Worked examples:

```tsx
style("text-blue-500", a && "text-red-500");
// a is true: red wins - equal specificity, the conditional unit comes later.

style("hover:text-blue-500", a && "text-red-500");
// hovered: blue wins - the variant selector is more specific, so a later
// conditional unit cannot outrank it. Put the same variant on the conditional
// (`a && "hover:text-red-500"`) to change the outcome.

style("hover:text-blue-500", a && "hover:text-red-500");
// hovered: red wins - same specificity, the conditional unit comes later.
```

Practical guidance:

- Keep utilities that must compete at equal specificity in the same call, so
  the canonical variant order (L2) applies to them.
- Use composition when the derived style must win over its base regardless of
  the base's specificity.
- Do not rely on the order of utilities inside one string to override a variant
  selector; specificity decides that case.

## Verifying a build

- Review fixtures compare declaration order, selector blocks, and module order
  for the pinned review projects.
- Property-order parity compares every interfering declaration pair in the
  Tailwind-generated fixture cases against Tailwind's own output; the harness
  fails the run on any mismatch. The current fixture set checks about 12,000
  candidates and 523 interfering-property constraints.
- Variant generation order is compared against fixtures generated from Tailwind
  v4 whose candidates are deliberately spelled out of canonical order, so the
  check cannot pass by preserving the input order.
- `vp run verify:composition` (inside `experiments/module-graph`) drives a real
  browser through development, production, reverse lazy-route order, and
  SSR/hydration, and confirms that base CSS precedes derived CSS.
- `vp run verify:conditional` covers nested conditions, SSR/hydration class
  equality, and content reload.
