---
title: Element relationships
description: Style an element from another element's state, size, or position through refs that TypeScript checks.
slug: docs/guides/element-relationships
---

You want to style an element based on another element: a parent's hover state,
a sibling's state, the width of a container, or the position of an anchor.
Tailwind connects the two elements by name, such as `group/card` and
`group-hover/card:`. It reads these names as strings, so it does not check that
they match. Nested groups or containers may also need names to target the right
ancestor. You must avoid name collisions when nesting components.

In Stillwind, you create the name as a variable with `style.ref()`. A misspelled
ref is an undefined variable, so TypeScript reports it.

## React to a parent's state

Create a ref, attach it to the parent, and use it in the child's style:

```tsx
import { style } from "stillwind/react";

const card = style.ref("card");

export function Card() {
  return (
    <article {...style(card, "rounded-lg p-4")}>
      <h2 {...style({ [card("hover")]: "text-blue-600" })}>Project</h2>
    </article>
  );
}
```

`style(card, ...)` attaches the ref to the `<article>`. The key `card("hover")`
applies `text-blue-600` while the `<article>` is hovered. Write the variants
inside the call, such as `card("hover:active")`, to combine states.

The label `"card"` only helps you read the generated class names. Each
`style.ref()` call creates its own name, so two components can use the label
`"card"` without affecting each other.

## React to a sibling's state

Without `.in()`, the ref must be on an ancestor of the styled element. To use a
sibling, add `.in()` with a ref on an element that contains both:

```tsx
import { style } from "stillwind/react";

const card = style.ref("card");
const trigger = style.ref("trigger");

export function Card() {
  return (
    <section {...style(card, "p-4")}>
      <button {...style(trigger, "rounded-md px-4 py-2")}>Details</button>
      <p {...style({ [trigger("hover").in(card)]: "text-blue-600" })}>
        This text changes color while the button is hovered.
      </p>
    </section>
  );
}
```

`trigger("hover").in(card)` matches while the `<button>` is hovered and both the
`<button>` and the `<p>` are inside the `<section>`.

## React to a container's size

Make the element a container with `asContainer()`, and query it with `at()`:

```tsx
import { style } from "stillwind/react";

const card = style.ref("card");

export function Card() {
  return (
    <section {...style(card, card.asContainer("inline-size"), "p-4")}>
      <p {...style({ [card.at("width >= 40rem")]: "text-lg" })}>
        This text grows when the card is at least 40rem wide.
      </p>
    </section>
  );
}
```

Pass `card` and `card.asContainer()` in the same `style()` call. The container
applies only to an element that has the ref, and the CLI warns when the ref is
missing.

## Position an element at an anchor

Make an element an anchor with `asAnchor()`, and position another element
against it with `positionAnchor()`:

```tsx
import { style } from "stillwind/react";

const trigger = style.ref("trigger");

export function AnchoredHint() {
  return (
    <section {...style(style.anchorScope([trigger]))}>
      <button {...style(trigger, trigger.asAnchor())}>Open hint</button>
      <div
        {...style(trigger.positionAnchor(), "absolute top-[anchor(bottom)] left-[anchor(left)]")}
      >
        Anchor-positioned content
      </div>
    </section>
  );
}
```

`style.anchorScope([trigger])` limits the anchor name to the `<section>`, so
each rendered `AnchoredHint` positions against its own button. Stillwind
generates CSS anchor positioning only. Check that the browsers you support
implement it.

## Share refs between components

Export a ref from its module and import it where you need it. Keep the call or
`.in()` chain, such as `card("hover")`, directly in the key of the `style()`
call. You cannot store it in a variable and use it later.

## Report group and peer variants

The `stylistic` preset includes
[prefer-typed-relationships](/docs/lints/prefer-typed-relationships). The rule
reports `group-*` and `peer-*` variants so that you can replace them with refs.

The [style.ref() reference](/docs/macros/ref) lists every method and its
accepted inputs.
