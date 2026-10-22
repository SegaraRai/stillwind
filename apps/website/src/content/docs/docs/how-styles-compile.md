---
title: How styles compile
slug: docs/how-styles-compile
description: See what Stillwind generates at build time, what JavaScript remains at runtime, and how CSS reaches the page.
---

Stillwind compiles `style()` calls when your bundler builds the application.
This page shows what the compiler generates, what JavaScript remains at
runtime, and how the CSS reaches the page.

## Static styles

A call with only static utilities compiles to an object literal:

```tsx
import { style } from "stillwind/react";

export const props = style("rounded-md p-4 text-blue-600");
```

The compiled JavaScript contains only the class name:

```js
export const props = { className: "sw_5lnfo" };
```

The compiler writes the CSS for that class to the application's stylesheet:

```css
.sw_5lnfo {
  padding: calc(var(--spacing) * 4);
  color: var(--color-blue-600);
  border-radius: var(--radius-md);
}
```

In this example, the three utilities compile to one class. Variants such as
`hover:` and `md:` also stay in CSS, so they add no JavaScript.

## Conditions and runtime values

A condition or a runtime value needs a small amount of JavaScript:

```tsx
import { style } from "stillwind/react";

declare const active: boolean;
export const props = style("p-4", active && "bg-blue-500", { "--progress": 0.5 });
```

The compiled call imports a helper that joins the class names, and it passes the
custom property as an inline style:

```js
import { stillwindClsx } from "stillwind/runtime/react";

export const props = {
  className: stillwindClsx("sw_5tqrt", active && "sw_35mvl"),
  style: { "--progress": "0.5" },
};
```

The CSS for both classes is generated at build time. At runtime, the code only
chooses class names and sets property values.

| You write                              | JavaScript at runtime                       |
| -------------------------------------- | ------------------------------------------- |
| Static utilities and variants          | A class name                                |
| A condition, such as `active && "..."` | A helper that joins class names             |
| A custom property                      | An inline `style` value                     |
| `style.cva()`                          | A factory that selects compiled class names |
| `style.media()`                        | A string with the media query               |

Development builds import checked versions of the helpers from
`stillwind/runtime/react-development`. These versions report invalid values while
you develop. Production builds use the unchecked helpers.

## The generated stylesheet

The stylesheet contains utility CSS only for the `style()` calls that the build
compiles. Utilities that no compiled call uses produce no CSS.

The compiler decides the order of the rules. The order does not depend on the
order of class names or on the order in which the bundler loads modules. The
rules have the same order in development, production, server, and client builds.

## Delivering CSS to the page

The build integration adds the generated CSS to your application. For Vite and
the frameworks built on it, the build writes one stylesheet by default.
The Astro integration also adds the styles to server-rendered pages and islands.
SvelteKit needs a server hook to add them to server-rendered pages in development.
See [Framework integrations](/docs/frameworks/) for the setup of each framework.

To place the CSS yourself, such as inside a shadow root, see
[Shadow DOM styles](/docs/guides/shadow-dom/).
