---
title: The basics of style()
description: Apply utilities, use variants and conditions, and compose reusable styles.
sidebar:
  order: 3
---

`style()` returns props for an element. The compiler reads its utility inputs
and emits the corresponding CSS. This page uses React; choose your framework's
entrypoint from [Framework integrations](/docs/frameworks/).

## Apply utilities

Spread the result onto an element:

```tsx
import { style } from "stillwind/react";

<section {...style("flex flex-col gap-4 rounded-lg bg-white p-6")}>
  <h2 {...style("text-xl font-semibold")}>Your workspace</h2>
  <p {...style("text-gray-600")}>Keep your component styles together.</p>
</section>;
```

Utilities resolve against the stylesheet selected by `stillwind.config.ts`.
Custom theme tokens become available through their corresponding utilities,
such as `bg-accent` for `--color-accent`.

## Add variants

Use variants inside the utility string for states and responsive rules:

```tsx
import { style } from "stillwind/react";

<button
  {...style(
    "rounded-md bg-accent px-4 py-2 text-white hover:opacity-90 focus-visible:outline-2 md:px-6",
  )}
>
  Save changes
</button>;
```

The compiler emits selectors and media queries for these static variants.
Their availability depends on the definitions in your project's stylesheet.

## Choose styles with conditions

Use a condition to select complete utility strings:

```tsx
import { style } from "stillwind/react";

export function SaveButton({ loading }: { loading: boolean }) {
  return (
    <button
      disabled={loading}
      {...style("rounded-md bg-accent px-4 py-2 text-white", loading && "cursor-wait opacity-60")}
    >
      {loading ? "Saving…" : "Save changes"}
    </button>
  );
}
```

The compiler can determine both utility branches while the condition selects
the active result at runtime. Keep utility construction static; avoid creating
utility names from unrestricted runtime strings.

## Compose completed styles

Store a completed `style()` result and pass it into another call:

```tsx
import { style } from "stillwind/react";

const button = style("rounded-md px-4 py-2 font-medium");
const primaryButton = style(button, "bg-accent text-white");

<button {...primaryButton}>Save changes</button>;
```

Completed results can be exported, passed to functions, and shared across
modules. Store an inner result before composing it; direct nested calls such
as `style(style("p-4"), "text-sm")` are unsupported.

## Pass dynamic values as custom properties

For a runtime value, use a custom property and a static utility that reads it:

```tsx
import { style } from "stillwind/react";

export function Progress({ value }: { value: number }) {
  return (
    <div
      {...style("h-2 w-(--progress) rounded-full bg-accent", {
        "--progress": `${value}%`,
      })}
    />
  );
}
```

Here, the utility remains static and the custom property's value changes at
runtime. Keep the value within the range your component accepts; this example
expects a number from 0 to 100.

## Keep inputs pure

Calculate values and perform effects before calling `style()`. Macro inputs
must not perform I/O, update state, or depend on their evaluation order or count.
The compiler can omit unused expressions.

Invalid inputs and project policies produce diagnostics. Whether a finding
stops the build depends on the finding and configured lint severity. See the
[macro contracts](https://github.com/SegaraRai/stillwind/blob/main/docs/macro-contract.md)
for input rules, refs, animation declarations, CVA, and CSS ordering.
