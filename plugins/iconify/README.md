# @stillwind/plugin-iconify

Stillwind compiles Tailwind CSS v4 utilities into component props and static CSS.
It uses theme rules and compiler checks to keep styling code in order, even when
AI agents write much of it.

`@stillwind/plugin-iconify` adds Iconify icons as Stillwind utilities. It generates
CSS for the icons used by your modules and includes each icon's SVG once.

## Usage

Install the plugin and the icon sets that the project uses:

```bash
vp i -D @stillwind/plugin-iconify @iconify-json/mdi
```

Load the plugin in the project stylesheet:

```css
@import "stillwind";
@plugin "@stillwind/plugin-iconify";
```

Set up the stylesheet and build integration with the
[Stillwind package guide](https://github.com/SegaraRai/stillwind/blob/main/packages/core/README.md).

A utility names an icon as `icon-<set>--<icon>` or as `icon-[<set>--<icon>]`. Both spellings
select the same utility, and variants apply as usual. The `prefer-canonical-utilities` lint
suggests the named spelling:

```tsx
<span {...style("icon-mdi--close", "hover:icon-[mdi--close-circle]")} />
```

## Sizes

An icon is `1em` high by default, and its width keeps the icon's aspect ratio. A size utility
sets the height of the icon on the same element:

- `icon-<name>` uses the `--icon-size-<name>` theme token.
- `icon-full` uses `100%` of the parent's height.
- `icon-[<length>]`, `icon-[<percentage>]`, and `icon-(<custom-property>)` use the given value.

```css
@theme {
  --icon-size-lg: 1.5rem;
}
```

```tsx
<span {...style("icon-mdi--close icon-lg")} />
```

`icon-full` needs a parent with a definite height. In a parent whose height depends on its
content, the icon is 0 high. To size an icon by its width instead, override the height:
`icon-mdi--close w-full h-auto` fills the parent's width and keeps the icon's aspect ratio.
When other utilities, such as `size-full` or `max-w-full`, change the box, the SVG keeps its
aspect ratio and stays centered in the box.

The size does not reach descendants or the element's `::before` and `::after` boxes. Apply the
size with the same variant as the icon, such as `before:icon-mdi--close before:icon-lg`.
`--icon-size-*` is a theme namespace that accepts lengths and percentages, so lint reports
other token values. The prefix option renames the utilities but not the namespace.

Size values follow lint policies configured for `height`. For example,
`no-restricted-length-units` with `properties: ["height"]` checks their units, and
`no-arbitrary-spacing` with `properties: ["height"]` reports arbitrary sizes. Named
sizes such as `icon-lg` remain theme-token selections.

The SVG of each used icon becomes the `--icon-svg-<set>--<icon>` theme token.

## Icon sets

The plugin finds a set in this order:

1. The `collections` option of a configured instance.
2. The `@iconify-json/<set>` package.
3. The `@iconify/json` package, which contains every Iconify set.

The host resolves packages from the stylesheet that contains the `@plugin` statement. A change
to a set's JSON file reloads the design system. Editors complete the icons of configured sets
and of the `@iconify-json/*` packages in the dependencies of the project's `package.json`.
When `@iconify/json` is installed, editors also complete its set names, such as `icon-mdi--`,
and list the icons of a set after the input names it.

## Options

The options block of the statement sets these options:

```css
@plugin "@stillwind/plugin-iconify" {
  prefix: i;
  syntax: bracket;
  scale: 1.2;
  mode: mask;
}
```

- `prefix`: utility prefix without the trailing hyphen. Defaults to `icon`.
- `syntax`: spellings that select an icon. `named` accepts `icon-mdi--close`, `bracket` accepts
  `icon-[mdi--close]` as Tailwind's Iconify plugin does, and `both`, the default, accepts both.
- `scale`: icon height in `em` when no size utility applies. Defaults to `1`.
- `mode`: `mask` paints the text color through the icon's shape, and `background` paints the
  icon's own colors. `auto`, the default, selects `mask` for icons whose SVG uses
  `currentColor`.

A configured instance can also supply icon sets, as data, as an absolute JSON file path, or as
a function that returns data. It replaces the default instance in the `@plugin` statement:

```ts
import { iconify } from "@stillwind/plugin-iconify";
import { defineConfig } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "src/stillwind.css",
      files: ["src/**/*.{ts,tsx}"],
      plugins: [
        iconify({
          collections: {
            brand: "/abs/path/brand.json",
          },
        }),
      ],
    },
  ],
});
```

Options in the statement's block replace the options of the configured instance.
