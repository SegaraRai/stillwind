# Stillwind

Stillwind compiles Tailwind CSS v4 utilities into component props and static CSS.
It uses theme rules and compiler checks to keep styling code in order, even when
AI agents write much of it.

`stillwind` is the main Stillwind package. It provides CSS defaults, configuration
helpers, and style APIs for React, Preact, Solid, Vue, Svelte, Astro, and vanilla JavaScript.

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

Register the [integration for your build tool](https://github.com/SegaraRai/stillwind/blob/main/packages/integration/README.md).
For Vite, add `stillwind()` from `@stillwind/integration/vite` before the framework's
JSX transform. The integration reads the project configuration and delivers its CSS.

## Write styles

Import `style` from the entrypoint for your framework:
`stillwind/react`, `stillwind/preact`, `stillwind/solid`, `stillwind/vue`,
`stillwind/svelte`, or `stillwind/astro`.

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

## Customize and extend

The default stylesheet imports the theme, preflight, utility slot, builtins, and
templates. Custom CSS entries receive only the definitions they import. Individual
stylesheets are available as `stillwind/theme.css`, `stillwind/preflight.css`,
`stillwind/utilities.css`, `stillwind/builtins.css`, and `stillwind/templates.css`.

Use `stillwind/config` for project settings and lint presets. `stillwind/css`
provides helpers for explicit stylesheet placement, and `stillwind/module-css`
exposes the compiled CSS of a producer module.

- [Style API reference](https://github.com/SegaraRai/stillwind/blob/main/docs/macro-contract.md): refs, conditions, CVA, keyframes, and composition.
- [Project configuration](https://github.com/SegaraRai/stillwind/blob/main/docs/standalone-analysis.md#shared-project-configuration): scopes, lint rules, sorting, and namespaces.
- [CSS delivery](https://github.com/SegaraRai/stillwind/blob/main/docs/css-emission.md): SSR, explicit CSS values, and Shadow DOM.
- [CSS ordering](https://github.com/SegaraRai/stillwind/blob/main/docs/css-ordering.md): precedence when styles are composed.
