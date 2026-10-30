---
title: Migrating from CSS
description: Move styles from plain CSS, CSS Modules, preprocessors, or CSS-in-JS into style() calls and theme tokens.
slug: docs/guides/migrating-from-css
---

You want to move a project that styles elements with plain CSS, CSS Modules,
Sass, Less, or CSS-in-JS to Stillwind. Unlike a migration from Tailwind CSS,
you translate declarations into utilities. Check the pages in the browser after
each step, because the translation can change how elements look.

## Set up Stillwind next to the existing CSS

Install Stillwind and register the build integration as described in
[Getting started](/docs). Your existing stylesheets keep working while you
migrate.

`@import "stillwind"` includes the preflight reset, which changes the default
styles of elements such as headings and lists. To migrate in steps, leave out
the reset until the old styles are gone:

```css
@layer properties, theme, base, components, utilities;

@import "stillwind/theme.css" layer(theme);
@import "stillwind/utilities.css" layer(utilities);
@import "stillwind/builtins.css";
@import "stillwind/templates.css";
```

Stillwind's CSS is in cascade layers. A declaration outside a layer wins over a
declaration in a layer, whatever their specificity. While an old rule matches an
element, it overrides the utilities on that element. Remove the old rule when
you move its declarations.

## Move values into the theme

Collect the colors, spacing, font sizes, radii, and breakpoints that the old
CSS repeats, such as Sass variables or custom properties in `:root`. Define them
as `@theme` tokens by role:

```css
@theme {
  --color-surface: oklch(0.98 0 0);
  --color-accent: oklch(0.6 0.2 250);
  --breakpoint-tablet: 48rem;
}
```

Each token adds utilities, such as `bg-surface` and `tablet:`. Merge values that
mean the same thing. Keep values separate when they are equal by chance.
[Design systems](/docs/guides/design-systems) describes how to name tokens.

## Move rules into style()

Translate the declarations of each rule into utilities, and apply them with
`style()` on the element:

```css title="Button.module.css"
.button {
  padding: 0.5rem 1rem;
  border-radius: 0.375rem;
  background: var(--accent);
}

.button:hover {
  opacity: 0.9;
}

.button[disabled] {
  cursor: wait;
}
```

```tsx
import { style } from "stillwind/react";

export function Button({ loading }: { loading: boolean }) {
  return (
    <button
      disabled={loading}
      {...style("rounded-md bg-accent px-4 py-2 hover:opacity-90 disabled:cursor-wait")}
    >
      Save
    </button>
  );
}
```

Translate selectors into variants:

| Old CSS                                | Stillwind                                                              |
| -------------------------------------- | ---------------------------------------------------------------------- |
| `:hover`, `:focus-visible`, `:checked` | `hover:`, `focus-visible:`, `checked:`                                 |
| `@media (min-width: 768px)`            | A breakpoint variant, such as `md:`                                    |
| `[aria-expanded="true"]`, `[data-x]`   | `aria-expanded:`, `data-[x]:`                                          |
| `.parent:hover .child`                 | A ref, see [Element relationships](/docs/guides/element-relationships) |
| A toggled modifier class               | An attribute and its variant                                           |
| A value set from JavaScript            | A custom property in `style()`                                         |

A declaration without a matching utility can use an arbitrary property, such as
`[content-visibility:auto]`. When several elements need it, define a custom
utility instead.

## Replace global rules

- Keep resets and base element styles until you enable the preflight reset.
  Then remove the rules that it replaces.
- Move `@keyframes` next to the component that uses them with
  [style.keyframes()](/docs/macros/keyframes).
- Move styles for HTML that you do not control, such as rendered Markdown, into
  a custom utility. See [Recommended practices](/docs/guides/recommended-practices#avoid-separate-css).

## Replace CSS-in-JS

Move static styles from `styled()` components or `css` props into `style()`
calls. Express props that change styles with attributes and variants, as in
[Reusable styles](/docs/guides/reusable-styles). For a continuous value, set a
custom property in `style()`.

## Check the migrated code

Run the [CLI checks](/docs/guides/lint-adoption) over the migrated files, and
compare the pages with the old version in the browser. After you remove the last
old stylesheet, switch to `@import "stillwind"` and check the pages again.
