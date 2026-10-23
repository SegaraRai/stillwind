# @stillwind/plugin-iconify

Stillwind compiles Tailwind CSS v4 utilities into component props and static CSS.

`@stillwind/plugin-iconify` adds Iconify icons as Stillwind utilities. Only the icons
you use are included in your CSS.

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

Use `icon-<set>--<icon>` or `icon-[<set>--<icon>]` to select an icon. Variants apply as
usual. The `prefer-canonical-utilities` lint suggests the named spelling:

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
Use lengths or percentages for `--icon-size-*` values. These property names stay the
same when you customize the utility `prefix`.

Size values follow lint policies configured for `height`. For example,
`no-restricted-length-units` with `properties: ["height"]` checks their units, and
`no-arbitrary-spacing` with `properties: ["height"]` reports arbitrary sizes. Named
sizes such as `icon-lg` remain theme-token selections.

## Icon sets

The plugin finds a set in this order:

1. The `collections` option of a configured instance.
2. The `@iconify-json/<set>` package.
3. The `@iconify/json` package, which contains every Iconify set.

Install icon set packages in the project that contains your stylesheet. Changes to a
set's JSON file update its icons automatically. Editors complete the icons of configured sets
and of the `@iconify-json/*` packages listed in the project's `package.json` dependencies.
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
  hash-length: 4;
}
```

- `prefix`: utility prefix without the trailing hyphen, such as `icon` or `i`. Defaults to `icon`.
- `syntax`: spellings that select an icon. `named` accepts `icon-mdi--close`, `bracket` accepts
  `icon-[mdi--close]`, and `both`, the default, accepts both.
- `scale`: icon height in `em` when no size utility applies. Defaults to `1`.
- `mode`: `mask` paints the text color through the icon's shape, and `background` paints the
  icon's own colors. `auto`, the default, selects `mask` for icons whose SVG uses
  `currentColor`.
- `hash-length`: base36 hash length in generated CSS variable names. Accepts integers
  from `1` to `7` and defaults to `4`. Increase it when using many icons to reduce collisions.

For custom icon sets or TypeScript options, add `iconify(...)` to your project's
`plugins` list. Icon sets can be data, absolute JSON file paths, or functions that
return data. Keep `@plugin "@stillwind/plugin-iconify";` in your stylesheet:

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

Options in the stylesheet's `@plugin` block override the TypeScript options. Use
`hashLength` for the `hash-length` option in TypeScript, such as `iconify({ hashLength: 6 })`.
