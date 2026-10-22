---
title: Responsive styles
description: Name breakpoints once, and use them in utilities, authored CSS, and JavaScript.
slug: docs/guides/responsive-styles
---

You want components to change layout at the same widths. In Tailwind, a
JavaScript check such as `matchMedia("(width >= 52.5rem)")` copies the breakpoint
value, and the copy can drift from the CSS. In Stillwind, you name each
breakpoint once and use the name everywhere.

## Name breakpoints by layout role

Name breakpoints after the layout change that they cause, instead of a device
size:

```css
@import "stillwind";

@theme {
  --breakpoint-inline: 40rem;
  --breakpoint-sidebar: 52.5rem;
  --breakpoint-outline: 70rem;
}

.navigation {
  @apply flex flex-col gap-3;
  @variant inline {
    @apply flex-row;
  }
}
```

Each `--breakpoint-*` token adds a variant, such as `sidebar:`. The default
breakpoints, such as `md:`, remain available unless you remove them.

## Use breakpoints in components

```tsx
import { style } from "stillwind/react";

export function Layout() {
  return (
    <div {...style("flex flex-col gap-4 sidebar:flex-row")}>
      <nav {...style("sidebar:w-64")}>Navigation</nav>
      <main {...style("flex-1")}>Content</main>
    </div>
  );
}
```

## Use breakpoints in authored CSS

In a CSS rule, nest `@variant` with the breakpoint name, as in the `.navigation`
rule above. In an `@utility` definition, use the variant in `@apply` instead,
such as `@apply inline:flex-row`.

## Use breakpoints in JavaScript

`style.media()` returns the media query for a breakpoint name:

```ts
import { style } from "stillwind/react";

const sidebar = window.matchMedia(style.media("sidebar"));
const update = () => {
  document.documentElement.dataset.sidebar = String(sidebar.matches);
};
sidebar.addEventListener("change", update);
update();

export function dispose() {
  sidebar.removeEventListener("change", update);
}
```

The compiler replaces `style.media("sidebar")` with the query string, so the
JavaScript always matches the CSS. `matchMedia()` is a browser API, so call it on
the client. Remove the listener when its component unmounts.

To combine conditions, write them in one argument. To accept alternatives, pass
several arguments:

```ts
import { style } from "stillwind/react";

const enhanced = window.matchMedia(style.media("outline:motion-safe", "print"));
```

This query matches a viewport at least as wide as `outline` without a
reduced-motion preference, or a printed page. `style.media()` accepts media
conditions only. Element states such as `hover` are not media conditions.

## Report widths outside your breakpoints

The [no-arbitrary-breakpoints](/docs/lints/no-arbitrary-breakpoints/) rule
reports widths written as values, such as `min-[713px]:`. See
[Design systems](/docs/guides/design-systems/#report-values-outside-the-theme)
to enable it.

To respond to the width of a component instead of the viewport, use a container
query. See [Element relationships](/docs/guides/element-relationships/#react-to-a-containers-size).
