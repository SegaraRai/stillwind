---
title: Migrating from Tailwind CSS
description: Move a Tailwind CSS v4 project to Stillwind, from the stylesheet and build to components and configuration.
slug: docs/guides/migrating-from-tailwind
---

You want to move an existing Tailwind CSS v4 project to Stillwind. Your utilities,
variants, theme, and plugins keep working. What changes is where you write them:
in `style()` calls instead of `className` strings.
[Tailwind CSS compatibility](/docs/guides/tailwind-compatibility) lists the tested
plugins and the differences.

Tailwind scans your files for anything that looks like a class name. Stillwind
generates CSS only for the utilities in `style()` calls. A class in a plain
`className` string, or in HTML from another source, gets no CSS after the
migration. Move every utility that you want to keep into `style()`.

## Replace the stylesheet and the build plugin

In your stylesheet, replace `@import "tailwindcss"` with `@import "stillwind"`:

```css
@import "stillwind";

@theme {
  --color-brand: oklch(0.6 0.2 250);
}
```

Keep your `@theme`, `@utility`, `@custom-variant`, `@variant`, `@apply`, and
`@plugin` rules.

Replace the Tailwind build plugin, such as `@tailwindcss/vite`, with the
Stillwind integration. Then create `stillwind.config.ts`, as described in
[Getting started](/docs#configure-the-project), and set up your framework, as
described in [Framework integrations](/docs/integrations).

## Move classes into style()

Replace each `className` string with a `style()` call that you spread onto the
element:

```tsx
import { style } from "stillwind/react";

export function Button() {
  return <button {...style("rounded-md bg-brand px-4 py-2 text-white")}>Continue</button>;
}
```

Import `style` from the entrypoint of the framework that renders the element.
For a class that another stylesheet defines, pass it in a `class` field, such as
`style("p-4", { class: "external-card" })`.

A large codebase may have hundreds of `className` strings. Your coding agent can
write a one-off codemod for your repository that converts them in one pass.

## Replace class helpers

`style()` and `style.cva()` replace the libraries that Tailwind projects use to
build class strings:

| Library                             | Stillwind                                 |
| ----------------------------------- | ----------------------------------------- |
| `clsx`, `classnames`                | `style()` with conditions                 |
| `tailwind-merge`                    | Compose a base with new utilities         |
| `cva` from class-variance-authority | `style.cva()` with the same configuration |

### clsx and classnames

Pass conditions as arguments:

```tsx
import { style } from "stillwind/react";

export function SaveButton({ loading }: { loading: boolean }) {
  return (
    <button {...style("rounded-md bg-brand px-4 py-2 text-white", loading && "opacity-60")}>
      Save
    </button>
  );
}
```

Do not convert the object form of `clsx`. In `clsx({ "opacity-60": loading })`,
each key is a class name. In `style()`, each key of an object is a variant, such
as `"hover:"`. Rewrite each entry as `loading && "opacity-60"`.

### tailwind-merge

Remove `twMerge()` calls. Store the base style and pass it to another `style()`
call. The compiler emits the new utilities after the base's CSS. They override
competing declarations when the other cascade criteria, such as specificity
and importance, are equal. See [Override a style](/docs/guides/style#override-a-style)
and [CSS ordering](/docs/concepts/css-ordering).

### cva

Replace `cva(` with `style.cva(`, and import `VariantProps` from your framework's
entrypoint:

```tsx
import { style, type VariantProps } from "stillwind/react";

const button = style.cva("rounded-md px-4 py-2", {
  variants: {
    tone: {
      neutral: "bg-gray-100",
      primary: "bg-brand text-white",
    },
  },
  defaultVariants: {
    tone: "neutral",
  },
});

export function Button({ tone }: VariantProps<typeof button>) {
  return <button {...button({ tone })}>Continue</button>;
}
```

`variants`, `defaultVariants`, and `compoundVariants` work as in
class-variance-authority. Write each option value as a plain utility string,
without `&&` or a ternary. The props that the factory returns cannot be passed
to `style()` as a base. The [style.cva() reference](/docs/macros/cva) lists the
other differences.

For new components, consider state variants and `data-*` attributes instead of
options. See [Reusable styles](/docs/guides/reusable-styles).

## Move configuration

Some Tailwind settings move from the stylesheet to `stillwind.config.ts`:

| Tailwind                          | Stillwind                                    |
| --------------------------------- | -------------------------------------------- |
| `@config "./tailwind.config.js"`  | The `tailwind` field of the project          |
| `@import "tailwindcss" important` | The `important` field of the project         |
| `@source` and `source(...)`       | Not supported; see below                     |
| `blocklist`                       | The `no-restricted-utilities` lint rule      |
| `prefix(tw)`                      | Write utilities without `tw:`; see below     |
| `theme(...)` on the import        | Not supported; define the tokens in `@theme` |

The `tailwind` field accepts a Tailwind configuration object. Its `theme`,
`presets`, `plugins`, `darkMode`, and `important` keys work as in Tailwind.
Other keys have no effect. This field requires the `tailwindcss` package.

Set `important` in one place only. Setting both `important` and
`tailwind.important` is an error.

Stillwind does not need the `tw:` class prefix, because it generates unique class
names. Theme variables can still collide with other CSS in the page. To keep the
`--tw-` names of `prefix(tw)`, set `themePrefix: "--tw-"` on the project. The
prefix applies to the generated CSS, including `var(--color-brand)` in your
stylesheet and in arbitrary values, so you can keep `var(--tw-color-brand)` or use
the shorter name. JavaScript that reads or sets a theme variable by name must use
the prefixed name.

Stillwind does not support `@source`, including `@source inline()`, or a safelist,
because only `style()` calls generate utilities. Tailwind classes inside a
dependency also get no CSS. Write each utility that you need in a `style()` call
in your own code.

Stillwind ignores `@config`, `@source`, `@reference`, and import options in the
stylesheet, and the CLI reports them with
[ignored-tailwind-feature](/docs/lints/ignored-tailwind-feature). Remove them after
you move their settings. The same rule reports a v3 path in `theme()`, such as
`theme(colors.red.500)`. Use `--theme(--color-red-500)` instead.

Keep your `@plugin` rules and the `@import` rules of CSS packages.
[Tailwind CSS compatibility](/docs/guides/tailwind-compatibility#plugins-and-css-packages)
lists the packages that the test suite covers.

## Check the migrated code

Run the [CLI checks](/docs/guides/lint-adoption) over the migrated files. They
report utilities that Stillwind cannot resolve and Tailwind features that it
ignores. Then test the production build and your pages in the browser.
