# @stillwind/plugin-iconify

[Stillwind](https://stillwind.dev/) compiles Tailwind CSS v4 utilities into component props and static CSS.

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

Set up the stylesheet and build integration with the [Getting started](https://stillwind.dev/docs) guide.

Use `icon-<set>--<icon>` or `icon-[<set>--<icon>]` to select an icon. Variants apply as usual.
The `prefer-canonical-utilities` lint suggests the form without brackets:

```tsx
<span {...style("icon-mdi--close", "hover:icon-[mdi--close-circle]")} />
```

See [Iconify icons](https://stillwind.dev/docs/plugins/iconify) for sizes, icon sets,
options, custom icon sets, and SVG minification.
