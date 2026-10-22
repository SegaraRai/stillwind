---
title: Animations
description: Define keyframes in the component that uses them, and respect reduced-motion preferences.
slug: docs/guides/animations
---

You want to animate a component. In Tailwind, you write `@keyframes` in a global
stylesheet and register the animation in the theme. As the codebase grows, nobody
knows which component still uses which animation. In Stillwind, you define the
keyframes in the component that uses them.

## Define and apply keyframes

```tsx
import { style } from "stillwind/react";

const enter = style.keyframes({
  from: "opacity-0 translate-y-2",
  to: "opacity-100 translate-y-0",
});

export function Notice() {
  return (
    <aside {...style({ "motion-safe:": enter("duration-300 ease-out both") })}>
      Your changes are saved.
    </aside>
  );
}
```

`style.keyframes()` defines the frames with utilities. `enter(...)` applies the
animation, and its utilities set the animation's options. For example,
`duration-300` sets `animation-duration`.

The compiler generates a unique animation name, so two components can both
define an `enter` animation. When you delete the component, its keyframes are no
longer generated.

## Respect reduced motion

The `motion-safe:` key in the example applies the animation only when the user
has not asked for reduced motion. Without the animation, the element shows its
ordinary styles, so make sure that those styles show the final state.

To use a different animation for reduced motion, add a definition under a
variant:

```ts
import { style } from "stillwind/react";

const enter = style.keyframes({
  from: "opacity-0 translate-y-2",
  to: "opacity-100 translate-y-0",
  "motion-reduce:": {
    from: "opacity-0",
    to: "opacity-100",
  },
});
```

The definition under `motion-reduce:` replaces the whole animation. It does not
inherit frames from the ordinary definition. Use media and `supports-*` variants
here. Container queries are not supported.

## Share an animation

Export the keyframes from their module, and import them in other components.
Write the call that applies them, such as `enter("duration-300")`, directly in
`style()`. You cannot store that call in a variable and use it later.

The [style.keyframes() reference](/docs/macros/keyframes/) shows the generated
JavaScript and CSS.
