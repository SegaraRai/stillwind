# stillwind — Golden Case

This document fixes the intended authoring experience for `stillwind`. It is a
product contract, not yet an implementation specification: the syntax may gain
small ergonomic refinements, but the principles below should not be weakened.

Implementation status: the React compiler implements the examples in this
document except the repeat-render/unstable-key warning for view transitions and
directory-policy configuration for `unsafe`. Where JSX placement is visible in
one file it validates handle ancestry; component boundaries, portals, and
arbitrary `children` are intentionally left to the escape-hatch boundary.

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

## One DOM relationship handle

`style.handle()` creates the only handle kind for relationships between DOM
elements. It is an opaque, compile-time identity, not a runtime object or a
manual class name.

```tsx
const trigger = style.handle();
```

The DOM decides where this identity is attached and how elements relate. The
style declaration does **not** prescribe a component tree or a slot structure.

```tsx
const trigger = style.handle();

<div {...style(trigger.scope, "relative")}>
  <button
    {...style(trigger.anchor, "inline-flex items-center rounded-md px-3 py-2", {
      "aria-expanded": open,
    })}
  >
    More
  </button>

  <div
    {...style(
      trigger.position,
      "absolute position-area-block-end z-popover rounded-md border bg-surface p-3",
      { [trigger`&[aria-expanded=true]`]: "block" },
    )}
  >
    Menu
  </div>
</div>;
```

The relevant projections are:

```ts
trigger.anchor; // anchor-name: --stillwind-…
trigger.position; // position-anchor: --stillwind-…
trigger.scope; // anchor-scope: --stillwind-…
trigger.container("inline-size"); // container-name/type
trigger.at("width >= 40rem", "grid-cols-2"); // named @container rule
trigger`&:hover`; // selector rooted at this referenced element
```

`scope` is deliberately separate from `anchor`. It belongs on whichever
existing DOM element is the intended visibility boundary. This matters when an
anchor and its positioned element are siblings: putting the scope on the anchor
itself would hide that anchor from its sibling.

```tsx
const panel = style.handle();

<section {...style(panel.container("inline-size"), "rounded-xl p-4")}>
  <div {...style("grid gap-4", panel.at("width >= 40rem", "grid-cols-2"))} />
</section>;
```

The linter validates what it can from the JSX tree and handle graph. It reports
missing anchor/container providers, invalid scope boundaries, impossible
selector relationships, and accidental handle reuse. It does not pretend to
prove component-boundary, portal, or arbitrary `children` relationships.

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

## View transitions are named CSS assets, not DOM handles

View transitions are independent from element-to-element relationships, so they
do not use `style.handle()`.

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

## Escape hatch

Unsupported third-party DOM, deeply dynamic selectors, and similar cases are
explicitly opt-in and visible to review tooling.

```tsx
style.unsafe({
  selector: "& [data-vendor-widget] .tooltip",
  css: { zIndex: 100 },
  reason: "The vendor widget exposes no styling part or anchor.",
});
```

`unsafe` is lintable and can be prohibited in selected directories or CI. It is
an escape hatch for the exceptional 5%, never the normal styling path.
