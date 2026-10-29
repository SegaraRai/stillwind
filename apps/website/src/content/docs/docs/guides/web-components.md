---
title: Web Components
description: Add the compiled styles of a component module to its shadow root.
slug: docs/guides/web-components
---

You want to style the elements inside the shadow root of a Web Component. The
build integration adds compiled CSS to the page's stylesheet, but the page's
stylesheet does not reach into a shadow root. Import the compiled CSS of the
component's module from `stillwind/module-css`, and add it to the shadow root.

## Keep the styles out of the page

```ts title="vite.config.ts"
import { stillwind } from "@stillwind/integration/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [
    stillwind({
      automatic: false,
    }),
  ],
});
```

With `automatic: false`, modules do not add their styles to the page's
stylesheet. Each component adds its styles to its own shadow root instead. The
webpack and Rspack plugins accept the same option.

Keep the default when the application also styles elements outside shadow
roots. A module that imports `stillwind/module-css` never adds its styles to the
page. With the default, a module that only exports shared styles still adds them
to the page.

## Import the CSS of a module

```ts
import { css, parts } from "stillwind/module-css";
```

The compiler replaces this import in each module that contains it. The CSS
contains the styles of the `style()` calls in that module and the styles that
those calls compose from other modules. It also contains the theme values, the
preflight reset, and the other rules of your stylesheet.

The import provides the CSS in two forms:

- `css` is one string. Use it with an API that accepts stylesheet text, such as
  Lit's `static styles`. The default export is the same string.
- `parts` lists the pieces of the CSS in order. Pass it to `styleSheets()` from
  `stillwind/css`, which returns constructed stylesheets. Shadow roots in the
  same document share the stylesheets for equal pieces, such as the preflight
  reset.

## Add the CSS to the shadow root

Add the CSS where your framework sets up the shadow root.

### Lit

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

customElements.define("save-button", SaveButton);
```

### Vue

Export `styles` from a plain `<script>` block of a `.ce.vue` file.
`defineCustomElement()` adds these styles to the shadow root:

```vue title="SaveButton.ce.vue"
<script lang="ts">
import { css } from "stillwind/module-css";

export default {
  styles: [css],
};
</script>

<script setup lang="ts">
import { style } from "stillwind/vue";
</script>

<template>
  <button v-bind="style('px-4 py-2')">Save</button>
</template>
```

```ts
import { defineCustomElement } from "vue";

import SaveButton from "./SaveButton.ce.vue";

customElements.define("save-button", defineCustomElement(SaveButton));
```

### Svelte

Set `compilerOptions.customElement` to `true` in `svelte.config.js`. Then adopt
the stylesheets after the component mounts:

```svelte
<svelte:options customElement="save-button" />

<script lang="ts">
  import { styleSheets } from "stillwind/css";
  import { parts } from "stillwind/module-css";
  import { style } from "stillwind/svelte";
  import { onMount } from "svelte";

  const host = $host();

  onMount(() => {
    host.shadowRoot!.adoptedStyleSheets = styleSheets(parts, host.ownerDocument);
  });
</script>

<button {...style("px-4 py-2")}>Save</button>
```

### Solid

With `solid-element`, adopt the stylesheets in the component function:

```tsx
import { customElement } from "solid-element";
import { styleSheets } from "stillwind/css";
import { parts } from "stillwind/module-css";
import { style } from "stillwind/solid";

customElement("save-button", {}, (_props, { element }) => {
  const root = element.renderRoot as ShadowRoot;
  root.adoptedStyleSheets = styleSheets(parts, element.ownerDocument);

  return <button {...style("px-4 py-2")}>Save</button>;
});
```

### Vanilla JavaScript

```ts
import { styleSheets } from "stillwind/css";
import { parts } from "stillwind/module-css";
import { applyStyle, clearStyle, style } from "stillwind/vanilla";

class SaveButton extends HTMLElement {
  readonly #root = this.attachShadow({
    mode: "open",
  });
  readonly #button = document.createElement("button");

  constructor() {
    super();
    this.#button.textContent = "Save";
    this.#root.append(this.#button);
  }

  connectedCallback(): void {
    this.#root.adoptedStyleSheets = styleSheets(parts, this.ownerDocument);
    applyStyle(this.#button, style("px-4 py-2"));
  }

  disconnectedCallback(): void {
    clearStyle(this.#button);
  }
}

customElements.define("save-button", SaveButton);
```

## Compile a stylesheet for a few utilities

`style.css()` compiles the utilities that you pass into a CSS string. Use it for
a small stylesheet, such as styles for `:host`, when you do not need the
preflight reset or the other rules of your stylesheet:

```ts
import { style } from "stillwind/vanilla";

export const hostCss = style.css("block p-4", {
  selector: ":host",
});
```

The `selector` option chooses the element that the utilities style. The string
contains the theme values and keyframes that its utilities need, and it adds
nothing to the page's stylesheet. Add it to the shadow root in the same way as
the `css` string from `stillwind/module-css`, or create a stylesheet from it with
`CSSStyleSheet.replaceSync()`. The [style.css() reference](/docs/macros/css)
describes variants that choose their own element and the `template` option.

## Constraints

- The CSS of a module contains only the styles of that module and the styles
  that it composes from other modules. The `style()` calls of a component in
  another module belong to the CSS of that module, even when the component
  renders into the same shadow root. Keep the `style()` calls for one shadow root
  in the module that imports the CSS, or compose styles from other modules into
  those calls.
- Shadow roots share the stylesheets that `styleSheets()` returns. Do not change
  them: a change appears in every shadow root that adopts the same stylesheet.
- If a browser that you support lacks `adoptedStyleSheets`, add `css` in a
  `<style>` element instead.
