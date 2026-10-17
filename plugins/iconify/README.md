# @stillwind/plugin-iconify

Iconify icons as Stillwind utilities. The plugin defines a utility only for the icons that
modules use, and the compiled CSS contains each icon's SVG once.

## Usage

Install the plugin and the icon sets that the project uses:

```bash
pnpm add -D @stillwind/plugin-iconify @iconify-json/mdi
```

Load the plugin in the project stylesheet:

```css
@import "stillwind";
@plugin "@stillwind/plugin-iconify";
```

A utility names an icon as `icon-<set>--<icon>` or as `icon-[<set>--<icon>]`. Both spellings
select the same utility, and variants apply as usual. The `prefer-canonical-utilities` lint
suggests the named spelling:

```tsx
<span {...style("icon-mdi--close", "hover:icon-[mdi--close-circle]")} />
```

## Icon sets

The plugin finds a set in this order:

1. The `collections` option of a configured instance.
2. The `@iconify-json/<set>` package.
3. The `@iconify/json` package, which contains every Iconify set.

Packages are resolved from the project directory. A change to a set's JSON file reloads the
design system. Editors complete the icons of configured sets and of the `@iconify-json/*`
packages in the dependencies of the project's `package.json`.

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
- `scale`: icon height in `em`. The width keeps the icon's aspect ratio. Defaults to `1`.
- `mode`: `mask` paints the text color through the icon's shape, and `background` paints the
  icon's own colors. `auto`, the default, selects `mask` for icons whose SVG uses
  `currentColor`.

A configured instance can also supply icon sets, as data, as an absolute JSON file path, or as
a function that returns data. It replaces the default instance in the `@plugin` statement:

```ts
import { iconify } from "@stillwind/plugin-iconify";
import { defineConfig } from "stillwind/config";

export default defineConfig({
  plugins: [
    iconify({
      collections: {
        brand: "/abs/path/brand.json",
      },
    }),
  ],
});
```

Options in the statement's block replace the options of the configured instance.
