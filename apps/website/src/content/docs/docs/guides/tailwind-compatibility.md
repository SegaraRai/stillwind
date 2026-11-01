---
title: Tailwind CSS compatibility
description: Check which Tailwind CSS v4 features, plugins, and CSS packages work in Stillwind, and what Stillwind does differently by design.
slug: docs/guides/tailwind-compatibility
---

Stillwind supports the utilities, variants, theme, CSS directives, and plugins of
Tailwind CSS v4. To move a project, follow
[Migrating from Tailwind CSS](/docs/guides/migrating-from-tailwind). This page
lists what works, how Stillwind checks it, what Stillwind adds, and where it differs.

For the usage of Tailwind features, see the
[Tailwind CSS documentation](https://tailwindcss.com/docs). To check the CSS of a
utility or variant, use [Built-in utilities and variants](/docs/utilities-and-variants).

## What works

| Tailwind feature                        | Support                                                              |
| --------------------------------------- | -------------------------------------------------------------------- |
| Default theme and preflight             | Included in `@import "stillwind"`                                    |
| Utilities and variants                  | Supported, including arbitrary values, modifiers, and `!`            |
| `@theme`, `@utility`, `@custom-variant` | Supported                                                            |
| `@apply`, `@variant`                    | Supported                                                            |
| `--theme()`, `--spacing()`, `--alpha()` | Supported                                                            |
| `theme(--color-red-500)`                | Supported; v3 paths such as `theme(colors.red.500)` are not          |
| `@plugin` with JavaScript plugins       | Supported; see [Plugins and CSS packages](#plugins-and-css-packages) |
| `@import` of CSS packages               | Supported; see [Plugins and CSS packages](#plugins-and-css-packages) |
| Tailwind configuration                  | Set the `tailwind` field of the project in `stillwind.config.ts`     |

## How Stillwind checks compatibility

The test suite compares Stillwind with the Tailwind compiler. It runs both
compilers on the same input and fails when their results differ:

- For utilities and variants, it compares the generated CSS for each class in
  Tailwind's public class list and in its variant list.
- For plugins and CSS packages, it builds the same stylesheet with both compilers
  and compares the computed styles in Chrome.

The declarations inside a rule can stand in a different order from Tailwind's, as
[CSS ordering](/docs/concepts/css-ordering) describes. The order of two declarations
that can affect each other stays the same, so the computed styles do not change.

## Plugins and CSS packages

Tailwind CSS v4 plugins work in Stillwind in principle. The test suite runs the
following packages in Vite development and production builds and in Rspack
production builds. It also compares their output with Tailwind.

Load these packages with `@plugin`:

| Package                             | Example                                     |
| ----------------------------------- | ------------------------------------------- |
| `@tailwindcss/typography`           | `prose`, `prose-lg`, `prose-headings:`      |
| `@tailwindcss/forms`                | `form-input`, and the base styles of forms  |
| `daisyui`                           | `btn`, `card`, `badge`, themes              |
| `tailwindcss-animate`               | `animate-in`, `fade-in-50`                  |
| `tailwindcss-motion`                | `motion-preset-fade`, `motion-duration-700` |
| `tailwind-scrollbar`                | `scrollbar-thin`, `scrollbar-thumb-brand`   |
| `tailwindcss-radix`                 | `radix-state-open:`                         |
| `tailwindcss-react-aria-components` | `selected:`                                 |
| `@headlessui/tailwindcss`           | `ui-open:`                                  |
| `@iconify/tailwind4`                | `icon-[lucide--home]`                       |

For Iconify icons, Stillwind also provides the
[`@stillwind/plugin-iconify`](/docs/plugins/iconify) plugin.

Load these packages with `@import`:

| Package                      | Example                                |
| ---------------------------- | -------------------------------------- |
| `tw-animate-css`             | `animate-in`, `fade-in-50`             |
| `tailwindcss-animated`       | `animate-fade`, `animate-duration-700` |
| `tailwindcss-intersect`      | `intersect:`                           |
| `tailwindcss-safe-area`      | `pt-safe-or-4`                         |
| `tailwind-scrollbar-hide/v4` | `scrollbar-hide`                       |
| `preline/variants.css`       | `hs-dropdown-open:`                    |

Other plugins can use the same features. [@plugin](/docs/at-rules/plugin#tailwind-css-plugins)
lists the supported plugin API. JavaScript plugins need the `tailwindcss` package,
and CSS packages do not.

The tests cover CSS. They do not run the JavaScript of a package, such as the
components of Preline. Plugins have no browser test with webpack or Next.js. Test
these combinations in your application.

### Class names that a plugin's selectors use

A plugin can select an element by a fixed class name. For example, the rules of
`prose` skip elements inside `not-prose`. `style()` keeps such a class in the
`class` attribute, so `style("not-prose")` adds the class `not-prose`. The class
keeps its name only without variants, so
[invalid-marker-use](/docs/lints/invalid-marker-use) reports `hover:not-prose`.

## What Stillwind adds

These features have no equivalent in Tailwind CSS:

| Feature                                 | Page                                                                        |
| --------------------------------------- | --------------------------------------------------------------------------- |
| `if()` variant and `--if()` function    | [Runtime conditions](/docs/guides/runtime-conditions)                       |
| `host:` variant                         | [Style the shadow host](/docs/guides/web-components#style-the-shadow-host)  |
| Keyframes defined in a component        | [Animations](/docs/guides/animations)                                       |
| View transitions defined in a component | [View transitions](/docs/guides/view-transitions)                           |
| Refs to other elements                  | [Element relationships](/docs/guides/element-relationships)                 |
| Logical properties for axis utilities   | [Logical axis utilities](/docs/concepts/stylesheets#logical-axis-utilities) |
| Lint rules                              | [Checks in the editor and CI](/docs/guides/lint-adoption)                   |

## What Stillwind does not support

These differences come from the design of Stillwind. No setting changes them.

### Classes outside style()

Only `style()` calls generate utilities. Stillwind does not scan files, so these
features have no replacement:

- `@source`, `@source inline()`, and `source(...)` on `@import`
- A safelist of class names
- Tailwind classes inside a dependency
- Utility classes that JavaScript adds at runtime, such as `` `bg-${color}-600` ``

Write each utility that you need in a `style()` call in your own code. For HTML
that you do not control, see
[Avoid separate CSS](/docs/guides/recommended-practices#avoid-separate-css).

### The tw: class prefix

Stillwind generates unique class names, so utilities do not need a prefix. Write
`p-4` instead of `tw:p-4`. Theme variables can still collide with other CSS in the
page. To prefix them, set `themePrefix` on the project, as
[Migrating from Tailwind CSS](/docs/guides/migrating-from-tailwind#move-configuration)
describes. In a plugin, `prefix()` returns the class name unchanged.

### Other differences

| Tailwind                                   | Stillwind                                                                         |
| ------------------------------------------ | --------------------------------------------------------------------------------- |
| `@reference`                               | Ignored; `@import` the stylesheet from the stylesheet of the project              |
| `theme(colors.red.500)`                    | Stays in the CSS, where browsers reject it; use `--theme(--color-red-500)`        |
| `blocklist`                                | Has no effect; use the `no-restricted-utilities` lint rule                        |
| Utilities that generate invalid CSS        | Rejected, such as `justify-baseline`, because `justify-content` has no such value |
| `addVariant()` with the v3 `:merge()` form | Skipped, as in Tailwind CSS v4                                                    |

[ignored-tailwind-feature](/docs/lints/ignored-tailwind-feature) reports the
features in your stylesheet that have no effect.
