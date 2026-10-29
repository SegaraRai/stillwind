---
title: Framework integrations
slug: docs/integrations
description: Import style() for your framework, and add Stillwind to your build tool.
sidebar:
  order: 2
---

Each application needs two things: the `style` import for its component framework,
and the Stillwind integration for its build tool. This page assumes that you have
created the stylesheet and `stillwind.config.ts` from [Getting started](/docs/).
Each integration finds `stillwind.config.ts` in the application root.

## Import style() for your framework

Import `style` from the entrypoint of the framework that renders the element:

| Framework          | Import              | Class prop  |
| ------------------ | ------------------- | ----------- |
| Alpine             | `stillwind/alpine`  | `class`     |
| Astro              | `stillwind/astro`   | `class`     |
| Lit                | `stillwind/lit`     | `class`     |
| Preact             | `stillwind/preact`  | `class`     |
| Qwik               | `stillwind/qwik`    | `class`     |
| React              | `stillwind/react`   | `className` |
| Solid              | `stillwind/solid`   | `class`     |
| Svelte             | `stillwind/svelte`  | `class`     |
| Vanilla JavaScript | `stillwind/vanilla` | `class`     |
| Vue                | `stillwind/vue`     | `class`     |

Spread the result with your framework's syntax. In JSX, Svelte, and Astro, write
`{...style("...")}`. In Vue templates, write `v-bind="style('...')"`:

```vue
<script setup lang="ts">
import { style } from "stillwind/vue";
</script>

<template>
  <button v-bind="style('rounded-md bg-accent px-4 py-2 text-white')">Save changes</button>
</template>
```

A component uses the entrypoint of its own framework. For example, a React
island inside an Astro page imports `stillwind/react`.

Alpine uses the `stillwind` plugin from `stillwind/alpine`. Register it with
`Alpine.plugin(stillwind)` before `Alpine.start()`, then bind compiled props with
`x-sw="props"`. Write macro calls in JavaScript or TypeScript modules.

Lit uses `html` element bindings such as
`` html`<button ${applyStyle(style("px-4 py-2"))}>Save</button>` ``.
Import both helpers from `stillwind/lit`. To add the styles to a shadow root,
see [Web Components](/docs/guides/web-components/). The Lit directive supports
browser rendering; Lit SSR is unsupported.

Gatsby uses `stillwind/react`, and Fresh 2 with Vite uses `stillwind/preact`.
For Fresh, use Deno 2.9 or newer and register `fresh()` before `stillwind()` from
`@stillwind/integration/fresh` in the Vite configuration. The adapter keeps server
and island classes consistent. Fresh's legacy Builder is unsupported.

## Vanilla JavaScript

Import the DOM helpers with the macro:

```ts
import { applyStyle, clearStyle, style } from "stillwind/vanilla";

const button = document.querySelector("button")!;
applyStyle(button, style("rounded-md bg-accent px-4 py-2 text-white"));
```

Call `applyStyle` again to update the element. Call `clearStyle(button)` to remove
its Stillwind contribution. The helpers preserve existing classes and unrelated
inline styles, and restore overwritten inline values unless another writer has
changed them. Use a build integration to compile the macros and deliver CSS.

Import framework macros under different names when one module styles both framework
elements and DOM elements. Each macro returns props for its own target.

## Vite

Use the Vite plugin for Alpine, Lit, Preact, Qwik, React, Solid, Svelte, vanilla
JavaScript, and Vue applications:

```ts
import { stillwind } from "@stillwind/integration/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [stillwind(), react()],
});
```

Place `stillwind()` before the framework plugin, because Stillwind must compile
styles before the framework compiles JSX or templates.

Some plugins split routes or server functions before compilation, such as the
TanStack Start plugin. Place those plugins before `stillwind()`:
`[tanstackStart(), stillwind(), react()]`.

## Astro

Add the integration to `astro.config.ts`:

```ts
import { stillwind } from "@stillwind/integration/astro";
import { defineConfig } from "astro/config";

export default defineConfig({
  integrations: [stillwind()],
});
```

The integration also adds styles to server-rendered pages and islands. Keep your
other integrations, such as `@astrojs/react`.

## Next.js

Wrap your configuration in `next.config.mjs`:

```js
import { withStillwind } from "@stillwind/integration/next";

export default withStillwind({});
```

Pass your existing configuration to `withStillwind()`. The wrapper works with
both webpack and Turbopack, and it keeps `"use client"` and `"use server"`
directives.

The stylesheet includes the routes under `app/`, `pages/`, `src/app/`, and
`src/pages/`, and the modules
that they import with static import paths. A module that you import through a
computed path, or a route written in MDX, gets no styles. If Next.js runs from a
directory other than the application root, set the `root` option.

## SvelteKit

Add the Vite plugin before `sveltekit()`:

```ts
import { stillwind } from "@stillwind/integration/vite";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [stillwind(), sveltekit()],
});
```

In development, add the server hook so that server-rendered pages include their
styles before JavaScript runs. Create `src/hooks.server.ts`:

```ts
export { handle } from "@stillwind/integration/sveltekit/handle";
```

If you already have a `handle`, combine both with `sequence()` from
`@sveltejs/kit/hooks`.

## Nuxt

Add the module to `nuxt.config.ts`:

```ts
import { stillwind } from "@stillwind/integration/nuxt";

export default defineNuxtConfig({
  modules: [stillwind()],
});
```

The module sets up Vite and server rendering. Do not add the Vite plugin as well.

## Other build tools

| Build tool | Import                             | Registration                       |
| ---------- | ---------------------------------- | ---------------------------------- |
| webpack    | `@stillwind/integration/webpack`   | `plugins: [new StillwindPlugin()]` |
| Rspack     | `@stillwind/integration/rspack`    | `plugins: [new StillwindPlugin()]` |
| Rsbuild    | `@stillwind/integration/rsbuild`   | `plugins: [pluginStillwind()]`     |
| Turbopack  | `@stillwind/integration/turbopack` | Add `stillwindRules({ entries })`  |

With webpack and Vue, place `StillwindPlugin` before `VueLoaderPlugin`. With
standalone Turbopack, list every entry module of the application in `entries`.

The [build integration reference](https://github.com/SegaraRai/stillwind/blob/main/docs/integrations.md)
describes every option.
[How styles compile](/docs/concepts/how-styles-compile/) explains how the CSS reaches the page.
