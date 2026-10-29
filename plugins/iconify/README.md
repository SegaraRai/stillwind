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
[Getting started](https://stillwind.dev/docs/) guide.

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

### License notices

The CSS output starts with a license notice for each icon set whose icons it contains, such as
`/*! Material Design Icons | Pictogrammers | Apache 2.0 | ... */`. The notice contains the set's
name, author, and license from the set's metadata. A `@iconify-json/<set>` package supplies the
metadata in its `info.json` file. The `@iconify/json` package and custom JSON sets supply it in
their `info` field.

A custom set without metadata adds no notice. It does not use the metadata of an installed set
with the same name. Pass the `info` option to `svgDirectory()` to add a notice for a set of SVG
files.

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
`plugins` list. Icon sets can be data, absolute JSON file paths, functions that
return data, or SVG directories. Keep `@plugin "@stillwind/plugin-iconify";` in your stylesheet:

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

> [!NOTE]
> Options in the stylesheet's `@plugin` block override the TypeScript options. Use
> `hashLength` for the `hash-length` option in TypeScript, such as `iconify({ hashLength: 6 })`.
> The stylesheet cannot set `collections` or `svgo` (described below).

### Custom icon set format

A custom icon set uses the Iconify JSON format, which the `IconifyJSON` type of
`@iconify/types` describes. This is the same format as the `icons.json` file of an
`@iconify-json/*` package:

```json
{
  "prefix": "brand",
  "width": 24,
  "height": 24,
  "icons": {
    "logo": {
      "body": "<path fill=\"currentColor\" d=\"M4 4h16v16H4z\"/>"
    },
    "badge": {
      "body": "<circle cx=\"12\" cy=\"12\" r=\"10\" fill=\"#e11d48\"/>",
      "width": 32,
      "height": 32
    }
  },
  "aliases": {
    "logo-flipped": {
      "parent": "logo",
      "hFlip": true
    }
  }
}
```

- `icons` maps each icon name to its SVG content. `body` contains the elements inside the
  `<svg>` element, without the `<svg>` element itself.
- `left`, `top`, `width`, and `height` define the icon's `viewBox`. An icon can set them, and
  the top level sets the default values for every icon. Missing values default to `0`, `0`,
  `16`, and `16`.
- `aliases` maps additional names to an existing icon in `parent`. An alias can override the
  `viewBox` values and add `rotate` (quarter turns), `hFlip`, and `vFlip` transformations.
- The key in `collections` sets the set name in utilities, such as `icon-brand--logo`. The
  plugin does not use the `prefix` field to find the set.

Set names and icon names must consist of lowercase letters and digits, separated by single
hyphens. Utilities cannot select other names, such as `Logo` or `logo_dark`.

With the default `mode: auto`, an icon whose `body` contains `currentColor` paints the text
color, and the other icons keep their own colors. In the example, `logo` follows the text
color and `badge` stays red.

The plugin does not validate the data. A JSON file must contain valid JSON, but the plugin
treats an icon that it cannot find in the data as an unknown utility.

### Icon sets from SVG files

`svgDirectory()` creates an icon set from the SVG files in a directory. Each file becomes one
icon, and the file name without `.svg` becomes the icon name:

```ts
import { iconify, svgDirectory } from "@stillwind/plugin-iconify";

iconify({
  collections: {
    brand: svgDirectory("src/icons/brand"),
    ui: svgDirectory("src/icons/ui", {
      recursive: true,
      transform: (svg) => svg.replaceAll('fill="#000"', 'fill="currentColor"'),
    }),
  },
});
```

The directory can be an absolute path or a path relative to the project directory. Each file
must contain an `<svg>` element with a `viewBox` attribute, which sets the icon's dimensions.
The plugin keeps the `style` attribute of the `<svg>` element and its attributes whose names
start with `fill` or `stroke`, such as `fill-rule`. It drops the other attributes of the
`<svg>` element.

- `recursive`: reads subdirectories. The default icon name joins the path segments with `-`,
  such as `arrows-left` for `arrows/left.svg`. Defaults to `false`.
- `name`: maps the file path relative to the directory, without `.svg` and with `/` separators,
  to an icon name. Returning `undefined` skips the file.
- `transform`: rewrites the SVG text of each file before the plugin parses it.
- `info`: the set's name, author, and license in the format of the Iconify `info` field. The
  plugin uses it for the [license notice](#license-notices).

Loading fails when an icon name is invalid, two files have the same icon name, or a file has
no `viewBox`. The error names the file. Adding, removing, or editing an SVG file reloads the
icons while a development server or build watcher runs.

A function in `collections` receives the set name and the host's `read`, `list`, and `watch`
operations, so you can write similar loaders. Watch each file and directory that the function
reads, such as `context.watch(file)` and `context.watch(directory, "directory")`.

### SVG minification

The plugin minifies the SVG of each icon with [SVGO](https://svgo.dev/) before it embeds the
SVG in CSS. The `svgo` TypeScript option controls this:

- `true`, the default, uses `preset-default` with `multipass` enabled.
- An SVGO configuration object replaces that default configuration.
- `false` embeds the SVG without minification.

```ts
iconify({
  svgo: {
    multipass: true,
    plugins: ["preset-default", "removeDimensions"],
  },
});
```
