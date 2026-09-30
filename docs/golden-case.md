# stillwind — Golden Case

This document fixes the intended authoring experience for `stillwind`. It is a
product contract, not yet an implementation specification: the syntax may gain
small ergonomic refinements, but the principles below should not be weakened.

Implementation status: the React compiler implements the examples in this
document except the repeat-render/unstable-key warning for view transitions.
Today's JSX placement lint uses facts visible in one file; that is an
implementation limitation, not a source-language restriction.

## Core principles

### Declaration values are JavaScript values

Objects created by `style.ref()`, `style.keyframes()`, and
`style.viewTransition()` are ordinary JavaScript values. They may be exported,
imported, re-exported, passed to and returned from functions, stored in data
structures, and conditionally selected across module boundaries. Stillwind must
not narrow this valid composition merely because a compiler pass cannot yet
resolve a value locally.

The dividing line is identity creation, not value flow. A declaration-producing call has
a declaration-site identity. Re-evaluating that call denotes the same
Stillwind identity rather than allocating an unbounded sequence of CSS names.
An API restriction is justified only by invalid CSS semantics, a genuine API
invariant, or an attempt to create an unbounded universe of _distinct_
identities.

### Compile ahead of time; do not define the language around the compiler

Static compilation realizes Stillwind semantics ahead of time; it is not a
reason to reject otherwise meaningful JavaScript. The compiler reasons about a
finite symbolic candidate set, not necessarily one exact local value:

```ts
const group = condition ? groupBoundary1 : groupBoundary2;
// group ∈ { groupBoundary1, groupBoundary2 }
```

Where safe, CSS retains the set symbolically instead of expanding a Cartesian
product:

```css
:is(.s_group-one, .s_group-two):hover .s_target {
  /* … */
}
```

Correlated choices must remain correlated. For example, a ref whose boundary
differs by alternative must not be lowered to a boundary/ref cross-product that
could match a DOM relationship the source never denoted. The compiler
factors only proven-independent candidate sets.

Stillwind never generates CSS at runtime. Exact values may erase completely;
dynamic declaration selection can retain a small opaque runtime descriptor solely to
select already-generated classes or declarations. This is bounded value
selection, not runtime CSS generation.

## One static macro at the element

`style()` is the only normal entry point for styles. It is a compile-time macro:
it extracts every statically knowable branch, emits CSS, and returns framework
props. It never builds CSS at runtime.

```tsx
import { style } from "stillwind/react";

function SaveButton({ loading }: { loading: boolean }) {
  return (
    <button
      {...style(
        "inline-flex items-center rounded-md bg-accent px-4 py-2 text-white",
        "focus-visible:outline-2 focus-visible:outline-offset-2",
        loading && "opacity-60 cursor-wait",
        { "aria-busy": loading },
        { "hover:": "bg-accent/90" },
      )}
    >
      Save
    </button>
  );
}
```

The conceptual React output is:

```tsx
{
  className: stillwindClsx("sw_a1", loading && "sw_b2"),
  "aria-busy": loading,
}
```

All literal utility strings in all statically enumerable branches are compiled.
Arbitrary dynamic utility construction is rejected.

```tsx
style(active ? "bg-accent" : "bg-muted"); // valid
style(`bg-${color}`); // invalid
```

## CSS-first theme and typed utility vocabulary

The CSS theme is the source of truth. The compiler reads it to generate utility
vocabulary and optional TypeScript declarations.

```css
/* src/styles/stillwind.css */
@theme {
  --color-accent: oklch(0.62 0.19 255);
  --color-surface: oklch(0.98 0.01 255);

  --spacing-4: 1rem;
  --radius-md: 0.375rem;

  --breakpoint-sm: 40rem;
  --breakpoint-md: 48rem;

  --duration-fast: 120ms;
  --duration-normal: 180ms;
  --ease-standard: cubic-bezier(0.2, 0.8, 0.2, 1);
}

[data-theme="dark"] {
  --color-accent: oklch(0.75 0.16 255);
  --color-surface: oklch(0.18 0.02 255);
}
```

```tsx
style("bg-accent rounded-md px-4 sm:grid md:grid-cols-2");
```

Unknown tokens, breakpoints, variants, and utility spellings are compilation
errors. Generated `.d.ts` files improve editor feedback, but compilation is the
final authority.

Theme changes use ordinary CSS variable cascading. Components refer to semantic
tokens such as `bg-accent`; they do not generally carry their own theme branch.

## Attributes and variant wrappers

Attributes are real props, not classes. A key ending in `:` wraps its value in
that variant prefix. Arrays and nested wrappers compose from outermost to
innermost: `{ "hover:": ["text-red", { "[&:where(.mod)]:": "text-blue" }] }`
is equivalent to `hover:text-red hover:[&:where(.mod)]:text-blue`. Each utility
in a whitespace-separated string receives the prefix. Raw `&...` keys are
rejected; use an arbitrary variant such as `"[&:where(.mod)]:"` instead.

```tsx
style(
  "grid place-items-center size-10 rounded-full bg-accent text-white",
  { "data-state": state },
  { "aria-busy": loading },
  {
    "[&[data-state=error]]:": "bg-danger",
    "[&[aria-busy=true]]:": "opacity-70 cursor-wait",
    "focus-visible:": "outline-2 outline-offset-2 outline-accent",
  },
);
```

Known `aria-*` and `data-*` contracts are type-checked where they are declared.
State selectors should be preferred over JavaScript conditionals when they
express the same visual state.

Object syntax is intentionally **not** a `clsx` object form. This leaves every
ordinary object key available for an attribute or selector, without an
ambiguous class-name branch. To add classes not owned by Stillwind, use the
explicit `class` entry. Its value is passed through a `clsx`-compatible helper,
so strings, numbers, nested arrays, logical branches, and object dictionaries
all work.

```tsx
style("rounded-md bg-accent", {
  class: ["legacy-button", loading && "is-loading", { "legacy-enabled": enabled }],
  "aria-busy": loading,
  "hover:": "bg-accent/90",
});
```

Plain strings passed directly to `style()` remain Stillwind utility strings and
are rejected if they are not in the configured utility vocabulary.

## DOM refs and boundaries

`style.ref()` is an opaque compile-time identity for a DOM element. Without a
boundary it is the named, local equivalent of Tailwind's `group`: a selector rooted
at the ref can target its descendants.

```tsx
const trigger = style.ref("trigger");

<button {...style(trigger, "inline-flex")}>
  <span {...style({ [trigger`hover:`]: "text-accent" })}>More</span>
</button>;
```

A ref accepts a static variant sequence as either a tagged template or a call.
The final `:` inside that sequence is optional. These forms are equivalent:

```tsx
style({ [trigger`hover:active:`]: "text-accent" });
style({ [trigger("hover:active")]: "text-accent" });
style(`${trigger`hover:active`}:text-accent`);
```

An argument-free call tests ref presence without a state condition:

```tsx
style({ [trigger()]: "text-accent" });
style(`${trigger()}:text-accent`);
```

For a bounded ref, this requires a matching element within the boundary
(`.target:is(:where(.boundary):has(:where(.bounded)) *)`). For an unbounded ref,
the matching element must be an ancestor (`.target:is(:where(.ref) *)`).

The variants inside `trigger(...)` test the referenced element. Variants outside
it apply to the styled target. Both forms below test a hovered trigger and an
active target:

```tsx
style({ [trigger("hover")]: { "active:": "text-accent" } });
style(`${trigger`hover:`}:active:text-accent`);
```

Ref wrappers accept the same arrays, nested variants, conditional utility
strings, and animation uses as ordinary variant wrappers. Direct utility
templates can mix literal utilities and multiple ref variants:

```tsx
style(`p-4 md:${trigger("hover")}:active:text-accent`);
```

Interpolation is parsed at compile time; it does not stringify a ref at runtime.
Each substitution must be a direct ref-variant call or tag occupying a complete
variant segment, followed by `:`. Imported refs and finite conditional ref
choices use the existing TypeScript identity facts. Arbitrary JavaScript string
interpolation is not supported. **Following interpolated strings stored in
variables is deliberately outside the API**, including local constants and
cross-module strings:

```tsx
const classes = `${trigger`hover:`}:text-accent`;
style(classes); // Not supported: write the template directly inside style(...).
```

A ref becomes a boundary when another ref names it. The bounded ref permits its
reference and selector target to be siblings or otherwise independently nested within
the boundary element.

```tsx
const menu = style.ref("menu");
const trigger = style.ref(menu, "trigger");

<section {...style(menu, "relative")}>
  <button {...style(trigger, "inline-flex items-center rounded-md px-3 py-2")}>More</button>
  <div {...style({ [trigger`hover:`]: "block" })}>Menu</div>
</section>;
```

The boundary element must be attached to a strict ancestor of its bounded ref. When
JSX is visible in one file the linter verifies that invariant. The generated selector
for the sibling example has the form:

```css
.sw_<target-hash>:is(:where(.sw_menu_<hash>):has(:where(.sw_trigger_<hash>):hover) *) { ... }
```

Labels are optional. `style.ref()` emits `.sw_<hash>`; `style.ref("menu")` and
`style.ref(menu, "trigger")` emit `.sw_menu_<hash>` and `.sw_trigger_<hash>`. The
hash keeps these local identities deterministic and collision-checked, and every other
generated name of one declaration spells the same label.

Refs also own anchor positioning and container-query projections:

```tsx
const popover = style.ref();
const card = style.ref();

<div {...style(style.anchorScope([popover]), card, card.asContainer("inline-size"), "relative")}>
  <button {...style(popover, popover.asAnchor(), "inline-flex")}>More</button>
  <div {...style(popover.positionAnchor(), "absolute position-area-block-end")} />
  <div {...style({ [card.at("width >= 40rem")]: "grid-cols-2" })} />
</div>;
```

Attach the ref itself to the same element that uses `asAnchor()` or
`asContainer()`. These projections add the generated `anchor-name` or
`container-name`; they do not attach the ref's marker. `asAnchor(scope?)` can
also add `anchor-scope`. `asContainer(type?)` adds `container-type` only when a
type is supplied.

`style.anchorScope(scope)` emits CSS `anchor-scope` on its element. Its argument
is `"all"`, `"none"`, or a literal array of refs. Naming a ref in another ref's
declaration creates the structural boundary used by ref selectors.

`at(query?)` returns a variant for a computed key or a utility template
substitution. A string literal and a static tag accept the same query:

```tsx
style({ [card.at`width >= 40rem`]: "grid-cols-2" });
style(`${card.at("width >= 40rem")}:grid-cols-2`);
```

Omitting the query or providing empty text emits `@container <container-name>`
without a query condition or parentheses. These forms are equivalent:

```tsx
style({ [card.at()]: "grid" });
style({ [card.at("")]: "grid" });
style({ [card.at``]: "grid" });
```

The linter checks missing ref attachments and anchor/container providers. It
does not infer relationships across component boundaries, portals, or arbitrary
`children`.

## Local animation, written with utilities

Animation definitions use the same utility-string language rather than a
parallel object-CSS DSL. A local animation receives a generated keyframe name;
authors never write that name.

```tsx
const enter = style.keyframes({
  from: "opacity-0 translate-y-2 scale-98",
  to: "opacity-100 translate-y-0 scale-100",
});

const exit = style.keyframes({
  from: "opacity-100",
  to: "opacity-0 scale-98",
});

<section
  {...style("rounded-xl bg-surface p-6 shadow-xl", enter("duration-normal ease-standard both"), {
    "[&[data-state=closed]]:": exit("duration-fast ease-standard both"),
  })}
/>;
```

Keyframes may use `from`, `to`, percentages, and grouped percentages. Their
declarations are validated against the same design-token policy as normal
styles. Shared motion belongs in `@theme`; component-specific motion stays next
to the component.

## View transitions are named CSS assets, not DOM refs

View transitions are independent from element-to-element relationships, so they
do not use `style.ref()`.

```tsx
const pageTitle = style.viewTransition({
  group: "duration-normal ease-standard",
  old: "opacity-0 -translate-y-1",
  new: "opacity-100",
});

<h1 {...style("text-3xl font-bold", pageTitle.for("hero"))}>Profile</h1>;
```

The declaration lowers to a value whose class token is the transition's
identity (`{v:"sw-vt-..."}`), and it styles the generated
`::view-transition-group`, `::view-transition-old`, and
`::view-transition-new` rules for that class. `imagePair` styles
`::view-transition-image-pair` for the same participants, and a phase utility
may carry media or supports condition variants. Other variants are rejected:
a pseudo-element has neither element state nor a container ancestor.

A use marks its element with the class, and `.for(key)` names it: a literal key
becomes a declaration, an expression key becomes an inline
`view-transition-name` composed as the token, a hyphen, and the key.
`view-transition-name` is a pairing key rather than an allocated name, so two
declarations may compose the same value on purpose, and the compiler neither
reserves nor checks name values.

Multiple simultaneously rendered instances require an application-stable key.

```tsx
const productCard = style.viewTransition({
  group: "duration-fast ease-standard",
  old: "opacity-0",
  new: "opacity-100",
});

<article {...style("rounded-xl border p-4", productCard.for(product.id))} />;
```

Uses of one declaration share its class, and several transitions in one call
list their classes in one declaration. Lint warns when a call marks
participants without any name (`require-view-transition-name`) and when it
states more than one (`single-view-transition-name`). A call that composes a base
declaring its own transitions replaces them, because `view-transition-class` is one
declaration, so `single-view-transition-class` warns about that replacement.
