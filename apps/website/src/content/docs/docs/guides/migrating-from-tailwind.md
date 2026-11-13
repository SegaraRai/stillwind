---
title: Migrating from Tailwind CSS
description: Move a Tailwind CSS v4 project to Stillwind, from the stylesheet and build to components and configuration.
slug: docs/guides/migrating-from-tailwind
---

You want to move an existing Tailwind CSS v4 project to Stillwind. Your utilities,
variants, theme, and plugins keep working. What changes is where you write them:
in [`style()`](/docs/guides/style) calls instead of `className` strings.
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

Keep your [`@theme`](/docs/at-rules/theme), [`@utility`](/docs/at-rules/utility),
[`@custom-variant`](/docs/at-rules/custom-variant), [`@variant`](/docs/at-rules/variant),
[`@apply`](/docs/at-rules/apply), and [`@plugin`](/docs/at-rules/plugin) rules.

Replace the Tailwind build plugin, such as `@tailwindcss/vite`, with the
Stillwind integration. Then create `stillwind.config.ts`, as described in
[Getting started](/docs#configure-the-project), and set up your framework, as
described in [Framework integrations](/docs/integrations).

With Tailwind, your code or framework configuration imports the stylesheet, for
example with `import "./app.css"` or in Starlight's `customCss`. With Vite and the
frameworks that run on it, remove these imports. The integration reads the
stylesheet that `css` selects and adds the compiled CSS to the page. If the
import remains, the page also receives the raw stylesheet.

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

[`style()`](/docs/macros/style) and [`style.cva()`](/docs/macros/cva) replace
the libraries that Tailwind projects use to build class strings:

| Library                                                                                           | Stillwind                                 |
| ------------------------------------------------------------------------------------------------- | ----------------------------------------- |
| [`clsx`](https://github.com/lukeed/clsx), [`classnames`](https://github.com/JedWatson/classnames) | `style()` with conditions                 |
| [`tailwind-merge`](https://github.com/dcastil/tailwind-merge)                                     | Compose a base with new utilities         |
| `cva` from [Class Variance Authority](https://cva.style/)                                         | `style.cva()` with the same configuration |

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
to `style()` as a base. The [`style.cva()` reference](/docs/macros/cva) lists
the other differences.

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
ignores. Then test the production build, and compare your pages with the old
version as described in the next section.

The `conflicting-utilities` lint rule can report a plugin's base class together
with its modifier classes, such as `btn btn-outline` in daisyUI. The
[Plugins](/docs/lints/conflicting-utilities#plugins) section of the rule page
lists a setting for each tested plugin that stops these reports.

### Compare computed styles

A successful build and clean CLI checks do not prove that the pages look the
same. For example, a class outside `style()` gets no CSS, and no check reports
it. A diff of the generated CSS or HTML does not help either, because Stillwind
renames the classes and reorders the rules. Instead, compare the computed styles
of the migrated components in a build from before the migration and a build from
after it. The test suite checks plugins in the same way, as
[Tailwind CSS compatibility](/docs/guides/tailwind-compatibility#how-stillwind-checks-compatibility)
describes.

1. Build the project before and after the migration, and serve both builds.
2. Open the same page in both builds, with the same viewport size, theme, and
   other state.
3. In each build, run the following function in the browser console. Pass a
   selector that matches the root elements of the migrated components. In
   Chrome DevTools, `copy(await snapshotStyles(".card"))` copies the result.
4. Save the two results to files, and compare them with a diff tool, such as
   `git diff --no-index before.txt after.txt`.

```js
async function snapshotStyles(rootSelector) {
  await document.fonts.ready;
  // Finishing a transition can start another one in a descendant that
  // inherits the value, so repeat until no animation runs.
  for (let round = 0; round < 20; round++) {
    document.documentElement.getBoundingClientRect();
    const running = document
      .getAnimations()
      .filter((animation) => animation.playState === "running");
    if (running.length === 0) {
      break;
    }
    for (const animation of running) {
      if (animation.effect.getComputedTiming().endTime === Infinity) {
        animation.pause();
        animation.currentTime = 0;
      } else {
        animation.finish();
      }
    }
  }
  const lines = [];
  document.querySelectorAll(rootSelector).forEach((root, rootIndex) => {
    [root, ...root.querySelectorAll("*")].forEach((element, index) => {
      for (const pseudo of ["", "::before", "::after"]) {
        const style = getComputedStyle(element, pseudo);
        if (pseudo && style.content === "none") {
          continue;
        }
        for (const name of [...style].filter((name) => !name.startsWith("--")).sort()) {
          const value = style.getPropertyValue(name);
          lines.push(`${rootIndex}.${index} ${element.localName}${pseudo} ${name}: ${value}`);
        }
      }
    });
  });
  return lines.join("\n");
}
```

Each line of the result contains the position of the element, the element or
pseudo-element, and one property with its value. The function finishes running
transitions and animations, and stops infinite animations at their start, so
a theme change just before the call does not leave intermediate values. It
skips custom properties, because their names usually change in a migration.

Repeat the comparison for each theme and for each breakpoint that the project
uses. Then review the differences:

- Some differences do not change the look. For example, when you move a margin
  from one element to its neighbor, the lines of both elements differ, but the
  layout stays the same.
- The function reads only the current state. To compare states such as `:hover`
  and `:focus-visible`, force the state in the browser's developer tools, or
  hover and focus the element with a browser automation tool such as Playwright.
- The function matches elements by their order inside each root. When the
  migration adds or removes an element, the lines that follow it no longer
  match, so compare that part by hand.
