---
title: Shadow DOM styles
description: Compile utilities into a stylesheet string, and add it to a shadow root yourself.
slug: docs/guides/shadow-dom
---

You want to style elements inside a shadow root, such as in a web component.
Styles in the page's stylesheet do not reach into a shadow root. Use
`style.css()` to compile utilities into a CSS string, and add that string to the
shadow root yourself.

## Compile a stylesheet string

```ts
import { style } from "stillwind/react";

export const css = style.css("p-4 text-blue-600", { selector: ":host" });
```

The compiler replaces the call with a string of CSS at build time. The
`selector` option chooses the element that the utilities style. Here, `:host`
styles the shadow host itself.

A variant that sets its own selector, such as `host:` or `[button]:`, chooses
its element instead. A utility without `selector` and without such a variant
produces no CSS, and the CLI reports it with
[missing-document-target](/docs/lints/missing-document-target/).

The string contains the theme values, keyframes, and other supporting CSS that
its utilities need. It does not contain the preflight reset,
and it adds nothing to the page's stylesheet.

## Add the stylesheet to the shadow root

```ts
import { css } from "./styles";

const element = document.createElement("example-card");
const root = element.attachShadow({ mode: "open" });
const sheet = new CSSStyleSheet();
sheet.replaceSync(css);
root.adoptedStyleSheets = [...root.adoptedStyleSheets, sheet];
root.textContent = "Styles are local to this shadow root.";
document.body.append(element);
```

Run this code where your component normally sets up its shadow root. If a
browser that you support lacks `adoptedStyleSheets`, add the string in a
`<style>` element instead.

The [style.css() reference](/docs/macros/css/) shows the generated string and
the `template` option.
