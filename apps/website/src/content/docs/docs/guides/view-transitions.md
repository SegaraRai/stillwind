---
title: View transitions
description: Style view transition snapshots in the component, and give each item a stable name.
slug: docs/guides/view-transitions
---

You want an item to animate between two states of the page with the browser's
[View Transition API](https://developer.mozilla.org/docs/Web/API/View_Transition_API).
In plain CSS, you write
[`view-transition-name`](https://developer.mozilla.org/docs/Web/CSS/Reference/Properties/view-transition-name)
and the `::view-transition-*` rules in a global stylesheet, and you keep every
name unique by hand. In Stillwind, you define the transition in the component
and name each item with a key.

## Define the transition and name each item

```tsx
import { style } from "stillwind/react";

export function Product({ id, name }: { id: string; name: string }) {
  const productTransition = style.viewTransition({
    old: "duration-150 motion-reduce:duration-0",
    new: "delay-150 duration-150 motion-reduce:delay-0 motion-reduce:duration-0",
  });
  return <article {...style(productTransition.for(id), "rounded-lg p-4")}>{name}</article>;
}
```

[`style.viewTransition()`](/docs/macros/view-transition) defines the styles of
the snapshots. Each field styles one pseudo-element:

| Field       | Pseudo-element                                                                                                                  |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `group`     | [`::view-transition-group()`](https://developer.mozilla.org/docs/Web/CSS/Reference/Selectors/::view-transition-group)           |
| `imagePair` | [`::view-transition-image-pair()`](https://developer.mozilla.org/docs/Web/CSS/Reference/Selectors/::view-transition-image-pair) |
| `old`       | [`::view-transition-old()`](https://developer.mozilla.org/docs/Web/CSS/Reference/Selectors/::view-transition-old)               |
| `new`       | [`::view-transition-new()`](https://developer.mozilla.org/docs/Web/CSS/Reference/Selectors/::view-transition-new)               |

Timing utilities in these fields set animation properties. For example,
`duration-150` sets `animation-duration`.

`.for(id)` gives the element a view transition name from the key. Use a key
that stays the same for the same item before and after the update, such as a
database ID. Two items on the page at the same time need different keys.

## Start the transition

Stillwind generates the styles and names. Your code starts the transition:

```ts
export function navigate(update: () => void) {
  if (document.startViewTransition) {
    document.startViewTransition(update);
  } else {
    update();
  }
}
```

Call `update` the way your framework normally renders a change. Browsers without
the View Transition API run the update without animation.

## Give each element one name

Use `.for()` once for each element, in one [`style()`](/docs/guides/style) call.
To combine several transitions on one element, pass the others without `.for()`:

```tsx
import { style } from "stillwind/react";

export function Card({ id }: { id: string }) {
  const fade = style.viewTransition({ old: "duration-150" });
  const slide = style.viewTransition({ new: "duration-300" });
  return <article {...style(fade, slide.for(id))}>Card</article>;
}
```

The CLI reports a `style()` call that applies a transition without `.for()`, a
call with two `.for()` calls, and transitions split across separate `style()`
calls. It cannot check that keys are unique across the whole page.

The [`style.viewTransition()` reference](/docs/macros/view-transition) covers
dynamic keys and the generated CSS.
