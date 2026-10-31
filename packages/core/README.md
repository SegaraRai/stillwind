# Stillwind

[Stillwind](https://stillwind.dev/) compiles Tailwind CSS v4 utilities into component props and static CSS.
It uses theme rules and compiler checks to keep styling code in order, even when AI agents write much of it.
See the [documentation](https://stillwind.dev/docs) and try it in the [Playground](https://play.stillwind.dev/).

`stillwind` is the main Stillwind package. It provides CSS defaults, configuration helpers, and style APIs for
Alpine, Astro, Lit, Preact, Qwik, React, Solid, Svelte, Vue, and vanilla JavaScript.

## Install

```sh
vp i stillwind
vp i -D @stillwind/integration
```

## Configure a project

Create a CSS entrypoint:

```css
/* src/stillwind.css */
@import "stillwind";

@theme {
  --color-accent: oklch(0.6 0.2 250);
}
```

Select that stylesheet and your source files in `stillwind.config.ts`:

```ts
import { defineConfig } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "./src/stillwind.css",
      files: ["src/**/*.{js,jsx,ts,tsx,vue,svelte,astro}"],
    },
  ],
});
```

Register the [integration for your build tool](https://stillwind.dev/docs/integrations).
For Vite, add `stillwind()` from `@stillwind/integration/vite` before the framework's
JSX transform. The integration reads the project configuration and delivers its CSS.

## Write styles

Import `style` from the entrypoint for your framework:

| Framework  | Import              | Applying props               |
| ---------- | ------------------- | ---------------------------- |
| Alpine     | `stillwind/alpine`  | `x-sw="props"`               |
| Astro      | `stillwind/astro`   | `{...props}`                 |
| Lit        | `stillwind/lit`     | `${applyStyle(props)}`       |
| Preact     | `stillwind/preact`  | `{...props}`                 |
| Qwik       | `stillwind/qwik`    | `{...props}`                 |
| React      | `stillwind/react`   | `{...props}`                 |
| Solid      | `stillwind/solid`   | `{...props}`                 |
| Svelte     | `stillwind/svelte`  | `{...props}`                 |
| Vue        | `stillwind/vue`     | `v-bind="props"`             |
| Vanilla JS | `stillwind/vanilla` | `applyStyle(element, props)` |

Gatsby uses `stillwind/react`. Fresh 2 with Vite uses `stillwind/preact`.

```tsx
import { style } from "stillwind/react";

const base = style("rounded-md bg-accent px-4 py-2 text-white");

<button {...style(base, loading && "opacity-60")}>Save</button>;
```

Style arguments must be pure. The compiler does not guarantee their evaluation
order or count, and it can omit unused evaluations. Perform effects outside style
calls. Static styles emit CSS; dynamic values and conditions can require runtime helpers.
Composed styles retain ordinary CSS specificity and importance.

For vanilla JavaScript, import the macro and DOM helpers from `stillwind/vanilla`:

```ts
import { applyStyle, clearStyle, style } from "stillwind/vanilla";

const button = document.querySelector("button")!;
applyStyle(
  button,
  style("rounded-md px-4 py-2", {
    "--accent": accent,
  }),
);
clearStyle(button);
```

Call `applyStyle` again to replace the element's previous Stillwind contribution.
The helper preserves existing classes and unrelated inline styles. `clearStyle`, or
`applyStyle(element, {})`, removes the contribution and restores overwritten inline
values and priorities. Inline values changed externally after the last application
are preserved. HTML and SVG elements use the same helpers. The build integration
delivers the generated CSS.

For Alpine, register the plugin before starting Alpine. Write macro calls in JavaScript
or TypeScript modules and pass their results to `x-sw`:

```ts
import Alpine from "alpinejs";
import { stillwind, style } from "stillwind/alpine";

Alpine.plugin(stillwind);
Alpine.data("button", () => ({
  active: false,
  get props() {
    return style("px-4 py-2", this.active && "bg-blue-500");
  },
}));
Alpine.start();
```

```html
<button x-data="button" x-sw="props" @click="active = !active">Toggle</button>
```

For Lit, pass the props to an element directive. Include the module's CSS in the
component's shadow root:

```ts
import { html, LitElement, unsafeCSS } from "lit";
import { applyStyle, style } from "stillwind/lit";
import { css } from "stillwind/module-css";

class SaveButton extends LitElement {
  static styles = unsafeCSS(css);

  render() {
    return html`<button ${applyStyle(style("px-4 py-2"))}>Save</button>`;
  }
}
```

Lit's directive supports browser rendering. Lit SSR is unsupported. Use
`stillwind({ automatic: false })` when styles belong only in shadow roots.
Alpine and Lit updates replace the helper's previous contribution and preserve
unrelated classes and inline styles. Removing the directive clears its contribution.
Lit reapplies its latest props when the directive reconnects.

Qwik uses ordinary JSX spreads. Keep Stillwind before the Qwik optimizer in the Vite
plugin list. Compiled props contain plain class and style values that Qwik can serialize.

## Customize and extend

The default stylesheet imports the theme, preflight, utility slot, builtins, and
templates. Custom CSS entries receive only the definitions they import. Individual
stylesheets are available as `stillwind/theme.css`, `stillwind/preflight.css`,
`stillwind/utilities.css`, `stillwind/builtins.css`, and `stillwind/templates.css`.
Import a file from `stillwind/logical/`, such as `stillwind/logical/overflow.css`,
after them to make the axis utilities of that property write logical properties.

Use `stillwind/config` for project settings and lint presets. `stillwind/css`
provides helpers for explicit stylesheet placement, and `stillwind/module-css`
exposes the compiled CSS of a producer module.

- [Macro reference](https://stillwind.dev/docs/macros): refs, conditions, CVA, keyframes, and composition.
- [Configuration](https://stillwind.dev/docs/configuration): scopes, lint rules, sorting, and namespaces.
- [CSS ordering](https://stillwind.dev/docs/concepts/css-ordering): precedence when styles are composed.
- [CSS delivery](https://github.com/SegaraRai/stillwind/blob/main/docs/css-emission.md): SSR, explicit CSS values, and Shadow DOM.
