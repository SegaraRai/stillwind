# stillwind — Golden Case

This document fixes the intended authoring experience for `stillwind`. It is a
product contract, not yet an implementation specification: the syntax may gain
small ergonomic refinements, but the principles below should not be weakened.

Implementation status: the React compiler implements the examples in this
document except the repeat-render/unstable-key warning for view transitions.
Where JSX placement is visible in one file it validates handle ancestry;
component boundaries, portals, and arbitrary `children` are intentionally not
inferred.

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
        { "&:hover": "bg-accent/90" },
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
  className: stillwindClsx("_s_a1", loading && "_s_b2"),
  "aria-busy": loading || undefined,
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

## Attributes and element-local selectors

Attributes are real props, not classes. A selector whose key starts with `&`
styles the same element.

```tsx
style(
  "grid place-items-center size-10 rounded-full bg-accent text-white",
  { "data-state": state },
  { "aria-busy": loading },
  {
    "&[data-state=error]": "bg-danger",
    "&[aria-busy=true]": "opacity-70 cursor-wait",
    "&:focus-visible": "outline-2 outline-offset-2 outline-accent",
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
  "&:hover": "bg-accent/90",
});
```

Plain strings passed directly to `style()` remain Stillwind utility strings and
are rejected if they are not in the configured utility vocabulary.

## DOM refs and scopes

`style.ref()` is an opaque compile-time identity for a DOM element. Without a
scope it is the named, local equivalent of Tailwind's `group`: a selector rooted
at the ref can target its descendants.

```tsx
const trigger = style.ref("trigger");

<button {...style(trigger, "inline-flex")}>
  <span {...style({ [trigger`&:hover`]: "text-accent" })}>More</span>
</button>;
```

`style.scope()` creates a DOM boundary. Binding a ref to that scope permits the
ref and selector target to be siblings or otherwise independently nested within
the boundary.

```tsx
const menu = style.scope("menu");
const trigger = style.ref(menu, "trigger");

<section {...style(menu, "relative")}>
  <button {...style(trigger, "inline-flex items-center rounded-md px-3 py-2")}>More</button>
  <div {...style({ [trigger`&:hover`]: "block" })}>Menu</div>
</section>;
```

The scope must be attached to a strict ancestor of its bound ref. When JSX is
visible in one file the linter verifies that invariant. The generated selector
for the sibling example has the form:

```css
.s_menu_<hash>:has(.s_trigger_<hash>:where(:hover)) .s_<target-hash> { ... }
```

Names are optional. `style.scope()` and `style.ref()` emit `.s_<hash>`;
`style.scope("menu")` and `style.ref(menu, "trigger")` emit
`.s_menu_<hash>` and `.s_trigger_<hash>`. The hash keeps these local identities
deterministic and collision-checked.

Refs also own anchor positioning and container-query projections:

```tsx
const popover = style.ref();

<div {...style(popover.anchorScope, popover.container("inline-size"), "relative")}>
  <button {...style(popover.anchor, "inline-flex")}>More</button>
  <div {...style(popover.position, "absolute position-area-block-end")} />
  <div {...style(popover.at("width >= 40rem", "grid-cols-2"))} />
</div>;
```

`anchorScope` is deliberately distinct from `style.scope()`: it emits CSS
`anchor-scope`, while `style.scope()` creates the structural boundary used for
ref selectors. The linter also checks missing anchor/container providers; it
does not infer relationships across component boundaries, portals, or arbitrary
`children`.

## Local animation, written with utilities

Animation definitions use the same utility-string language rather than a
parallel object-CSS DSL. A local animation receives a generated keyframe name;
authors never write that name.

```tsx
const enter = style.animation({
  from: "opacity-0 translate-y-2 scale-98",
  to: "opacity-100 translate-y-0 scale-100",
});

const exit = style.animation({
  from: "opacity-100",
  to: "opacity-0 scale-98",
});

<section
  {...style("rounded-xl bg-surface p-6 shadow-xl", enter("duration-normal ease-standard both"), {
    "&[data-state=closed]": exit("duration-fast ease-standard both"),
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

<h1 {...style("text-3xl font-bold", pageTitle)}>Profile</h1>;
```

This creates a stable generated `view-transition-name` and styles its generated
`::view-transition-group`, `::view-transition-old`, and
`::view-transition-new` rules.

Multiple simultaneously rendered instances require an application-stable key.

```tsx
const productCard = style.viewTransition({
  group: "duration-fast ease-standard",
  old: "opacity-0",
  new: "opacity-100",
});

<article {...style("rounded-xl border p-4", productCard.for(product.id))} />;
```

`for(product.id)` emits a static class plus the minimal dynamic custom property
needed for a unique transition name. Lint warns when a static transition is used
inside a repeated render, or when an unstable key such as an array index is
used.
