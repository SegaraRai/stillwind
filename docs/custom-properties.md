# Inline custom properties

`style()` accepts quoted `--*` fields alongside utility strings, attributes,
external classes, and variant wrappers. Values remain JavaScript expressions:
the compiler evaluates each once, in source order, and preserves conditional
short-circuiting.

```tsx
import imageUrl from "./image.png?url";
import { style } from "stillwind/react";

const props = style("bg-(image:--image) w-[calc(var(--progress)*100%)]", {
  "--image": `url("${imageUrl}")`,
  "--progress": progress,
});
```

The bundler processes the image import, including asset hashing and deployment
base handling. Stillwind does not resolve or rewrite these runtime values.
`bg-(image:--image)` explicitly selects background-image; an untyped background
variable can select the color utility instead.

| Value                                      | Inline CSS text                        |
| ------------------------------------------ | -------------------------------------- |
| `"1rem"`                                   | `1rem`                                 |
| `0.5`                                      | `0.5` (no automatic unit)              |
| `true` / `false`                           | One space / `initial`                  |
| `new URL("https://example.com/image.png")` | `url("https://example.com/image.png")` |

Strings are already CSS values. A `?url` import produces a string, so wrap it
in `url(...)` when using it as an image. URL objects receive CSS quoting and
escaping automatically. Supply valid CSS text for the property consuming a string.

```tsx
style({ "--progress": 0, "--size": "1rem" }, active && { "--progress": progress });
```

Boolean values act as switches for unregistered custom properties:

```tsx
style({ "--enabled": enabled });
```

```css
.button {
  --enabled-color: var(--enabled) green;
  color: var(--enabled-color, gray);
}
```

True supplies an empty CSS value, leaving `green`; false supplies `initial`,
making the intermediate variable invalid and selecting `gray`. A single space
preserves the empty value through CSSOM, where an empty string would remove the
property. The same switch can be queried with `if(style(--enabled): green; else: gray)`.
Registering the property with `@property` can change its initial-value semantics.
Use `Number(enabled)` when a numeric 1/0 is needed for calculations.

Properties merge independently; a later active entry with the same name wins.
Zero and false are values, not omitted entries. Conditional objects and arrays
work like other style inputs. Keys must be quoted literal names; computed keys
and object spreads retain their existing restrictions. Custom properties cannot
be nested inside CSS variant wrappers: put the variable on the element and let
the variant's utility reference it.

React, Preact, Solid, and Vue receive an object in the returned `style` prop.
Svelte and Astro receive a serialized style string. Custom properties overlay
the generated style payload, preserving an unrelated view transition name.
The existing restriction on composing props-bearing style results also applies
to results containing custom properties.
