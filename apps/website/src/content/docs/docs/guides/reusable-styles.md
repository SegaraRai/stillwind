---
title: Reusable styles
description: Share a base style, and express states and options with variants instead of toggled classes.
slug: docs/guides/reusable-styles
---

You want components to share styles and to change their look by state or by
option. In Stillwind, you share a base style, and you express states and options
with variants. The element keeps the same classes, and its attributes decide
which styles apply.

## Share a base style

Store a `style()` result, and pass it to other `style()` calls:

```tsx
import { style } from "stillwind/react";

export const button = style("inline-flex rounded-md px-4 py-2 font-medium");
const primaryButton = style(button, "bg-blue-600 text-white");
const dangerButton = style(button, "bg-red-600 text-white");

export function Actions() {
  return (
    <div {...style("flex gap-2")}>
      <button {...primaryButton}>Save</button>
      <button {...dangerButton}>Delete</button>
    </div>
  );
}
```

The later utilities win over the base, whatever the order of the class names.
You can export a base and import it in other modules.

A base passes on its classes only. Custom properties such as
`{ "--progress": value }` in the base do not carry over, and the CLI reports
them. Set custom properties in the `style()` call that is spread onto the element.

## Express states with variants

Set the state as an attribute, and style it with a variant:

```tsx
import { style } from "stillwind/react";

export function ToggleButton({ pressed }: { pressed: boolean }) {
  return (
    <button
      aria-pressed={pressed}
      {...style(
        "rounded-md bg-gray-100 px-4 py-2",
        "aria-pressed:bg-blue-600 aria-pressed:text-white",
      )}
    >
      Bold
    </button>
  );
}
```

The `aria-pressed` attribute is needed for accessibility anyway. Use variants
such as `disabled:`, `aria-expanded:`, and `open:` in the same way. The element
always has the same classes, so you do not need to think about which toggled
class wins.

## Express options with data attributes

For an option without a matching ARIA attribute, set a `data-*` attribute:

```tsx
import { style } from "stillwind/react";

export function Badge({ tone, label }: { tone: "neutral" | "danger"; label: string }) {
  return (
    <span
      data-tone={tone}
      {...style(
        "rounded-full bg-gray-100 px-2 text-gray-900",
        "data-[tone=danger]:bg-red-600 data-[tone=danger]:text-white",
      )}
    >
      {label}
    </span>
  );
}
```

The attribute also shows the option in the browser's developer tools and in
test queries.

For a value from a continuous range, such as a width or a progress percentage,
set a custom property instead. See
[Use runtime values](/docs/style/#use-runtime-values).

## Migrate CVA components

`style.cva()` accepts the configuration of `cva` from class-variance-authority.
Use it to migrate existing CVA components. See
[Replace class helpers](/docs/guides/migrating-from-tailwind/#replace-class-helpers)
and the [style.cva() reference](/docs/macros/cva/).
