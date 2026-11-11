---
title: The basics of style()
slug: docs/guides/style
description: Apply utilities, variants, conditions, overrides, and runtime values to an element.
sidebar:
  order: 3
---

`style()` takes Tailwind CSS v4 utilities and returns props for an element.
You spread the props onto the element instead of writing a `className` string.
This page uses React. For other frameworks, import `style` from the entrypoint
listed in [Framework integrations](/docs/integrations).

## Apply utilities

```tsx
import { style } from "stillwind/react";

export function Workspace() {
  return (
    <section {...style("flex flex-col gap-4 rounded-lg bg-white p-6")}>
      <h2 {...style("text-xl font-semibold")}>Your workspace</h2>
      <p {...style("text-gray-600")}>Keep your component styles together.</p>
    </section>
  );
}
```

The compiler reads the utilities at build time and generates their CSS.
In React, `style()` returns a `className` prop.

Utilities and variants come from the stylesheet that `stillwind.config.ts`
selects. A token that you add to [`@theme`](/docs/at-rules/theme) adds utilities,
such as `bg-accent` for `--color-accent`.

## Add variants

Write variants as in Tailwind:

```tsx
import { style } from "stillwind/react";

export function SaveButton() {
  return (
    <button {...style("rounded-md bg-blue-600 px-4 py-2 text-white hover:bg-blue-700 md:px-6")}>
      Save changes
    </button>
  );
}
```

To apply several utilities under the same variant, use an object. Each key is
a variant followed by a colon:

```tsx
import { style } from "stillwind/react";

export function Link() {
  return (
    <a href="/docs" {...style("text-blue-600", { "hover:": "text-blue-800 underline" })}>
      Documentation
    </a>
  );
}
```

### Match any of several variants

`or(...)` applies a utility when any of its variants matches. Separate the
variants with commas. Each one can contain several variants separated by colons:

```tsx
import { style } from "stillwind/react";

export function MenuItem() {
  return <li {...style("px-4 py-2 or(hover,focus-within,md:aria-current):bg-gray-100")}>Inbox</li>;
}
```

`or(hover,md:focus):p-4` is the same as `hover:p-4 md:focus:p-4`. Each
variant keeps the position in the stylesheet that it has when you write it alone,
so [CSS order](/docs/concepts/css-ordering) does not change. You can also use
`or(...)` in an object key, such as `{ "or(hover,focus):": "underline" }`.
To reuse alternatives that include
[ref conditions](/docs/macros/ref/condition), pass several arguments to
[`style.when()`](/docs/macros/when#alternatives).

## Style states

When the element has an attribute for the state, use a variant:

```tsx
import { style } from "stillwind/react";

export function SaveButton({ loading }: { loading: boolean }) {
  return (
    <button
      disabled={loading}
      {...style(
        "rounded-md bg-blue-600 px-4 py-2 text-white disabled:cursor-wait disabled:opacity-60",
      )}
    >
      {loading ? "Saving…" : "Save changes"}
    </button>
  );
}
```

The element keeps the same classes, and the browser applies the `disabled:`
styles while the button is disabled. [Reusable styles](/docs/guides/reusable-styles)
shows the same approach with `aria-*` and `data-*` attributes.

When no attribute represents the state, pass a condition and a string, as with `clsx`:

```tsx
import { style } from "stillwind/react";

export function Row({ selected }: { selected: boolean }) {
  return <li {...style("px-4 py-2", selected && "bg-blue-50 font-medium")}>Inbox</li>;
}
```

The compiler generates CSS for both strings. At runtime, the condition decides
whether the element gets `bg-blue-50 font-medium`.

Write complete utility names in the source. The compiler cannot generate CSS for
a name that your code builds at runtime, such as `` `bg-${color}-600` ``.
Choose between complete names instead, such as
`color === "red" ? "bg-red-600" : "bg-blue-600"`.

## Override a style

Store a `style()` result in a variable, and pass it to another `style()` call.
The compiler places CSS for the new utilities after the base's CSS:

```tsx
import { style } from "stillwind/react";

const button = style("rounded-md bg-blue-600 px-4 py-2 text-white");
const dangerButton = style(button, "bg-red-600");

export function DeleteButton() {
  return <button {...dangerButton}>Delete</button>;
}
```

In this example, `dangerButton` has a red background because both background
rules use equally specific selectors. The order of the class names does not
affect the result. You can export a stored style and use it in other modules.
When another module adds utilities that set the same property as the imported
style, [interfering-imported-style-base](/docs/lints/interfering-imported-style-base)
reports the call. Define the overriding style in the module that defines the
base, and import that style instead.

The order of the arguments does not affect the result either.
`style("bg-red-600", button)` also produces a red background, and
`style("p-4", "p-2")` produces the same CSS as `style("p-2", "p-4")`. To change
which utility wins, remove the other utility or build a new style on a base. Do
not reorder the arguments. See [Argument order](/docs/macros/style#argument-order)
for the exceptions.

More specific selectors and `!important` can change which declaration wins.
See [CSS ordering](/docs/concepts/css-ordering) for variants, conditions, and multiple
bases.

Store the inner style in a variable first. A `style()` call written directly
inside another, such as `style(style("p-4"), "text-sm")`, is not supported.
[Reusable styles](/docs/guides/reusable-styles) covers shared styles and
component options in more detail.

## Use runtime values

For a value that is known only at runtime, set a CSS custom property and read it
in a utility:

```tsx
import { style } from "stillwind/react";

export function Progress({ value }: { value: number }) {
  return (
    <div {...style("h-2 w-(--progress) rounded-full bg-blue-600", { "--progress": `${value}%` })} />
  );
}
```

The utility `w-(--progress)` stays the same, so its CSS is generated at build
time. Only the value of `--progress` changes at runtime. To change utilities with
a custom property, see [Runtime conditions](/docs/guides/runtime-conditions).

## Add classes from other stylesheets

To add a class that another stylesheet defines, pass it in a `class` field:

```tsx
import { style } from "stillwind/react";

export function Card({ className }: { className?: string }) {
  return <article {...style("rounded-lg p-4", { class: className })} />;
}
```

Stillwind passes these classes through and does not generate CSS for them.

## Keep arguments free of side effects

The compiler can evaluate the arguments of `style()` in any order, and it can
skip arguments whose result is not used. Calculate values and perform effects,
such as state updates or I/O, before you call `style()`.

## Next steps

- [Reusable styles](/docs/guides/reusable-styles): how to share styles and define component options
- [Responsive styles](/docs/guides/responsive-styles): how to name breakpoints and use them in JavaScript
- [Checks in the editor and CI](/docs/guides/lint-adoption): how to find misspelled utilities and other mistakes
- [The `style()` reference](/docs/macros/style): every accepted input
