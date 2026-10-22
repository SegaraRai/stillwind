---
title: Framework integrations
description: Connect Stillwind to your build and import the style API for your framework.
sidebar:
  order: 2
---

First create your CSS entrypoint and `stillwind.config.ts` as described in
[Getting started](/docs/). Each build adapter discovers that configuration from
the application root.

## Choose the style entrypoint

Use the entrypoint for the framework that renders the element:

| Framework | Import             | Generated class prop | Generated style value |
| --------- | ------------------ | -------------------- | --------------------- |
| React     | `stillwind/react`  | `className`          | Object                |
| Preact    | `stillwind/preact` | `class`              | Object                |
| Solid     | `stillwind/solid`  | `class`              | Object                |
| Vue       | `stillwind/vue`    | `class`              | Object                |
| Svelte    | `stillwind/svelte` | `class`              | String                |
| Astro     | `stillwind/astro`  | `class`              | String                |

Spread the returned props with your framework's normal syntax. In Vue templates,
use `v-bind="style(...)"`.

## Vite

React, Preact, Solid, Vue, and Svelte applications use the Vite adapter alongside
their framework plugin. For example, a React application's `vite.config.ts` is:

```ts
import { stillwind } from "@stillwind/integration/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [stillwind(), react()],
});
```

Keep Stillwind before the framework's JSX transform. Plugins that split routes
or server functions run before Stillwind. For TanStack Start, the order is
`[tanstackStart(), stillwind(), react()]`.

Vue templates can use the same API after the Vite adapter is registered:

```vue
<script setup lang="ts">
import { style } from "stillwind/vue";
</script>

<template>
  <button v-bind="style('rounded-md bg-accent px-4 py-2 text-white')">Save changes</button>
</template>
```

## Astro

Register the Astro integration in `astro.config.ts` and keep your other
integrations:

```ts
import { stillwind } from "@stillwind/integration/astro";
import { defineConfig } from "astro/config";

export default defineConfig({
  integrations: [stillwind()],
});
```

Import the Astro entrypoint in `.astro` components:

```astro
---
import { style } from "stillwind/astro";
---

<button {...style("rounded-md bg-accent px-4 py-2 text-white")}>Save changes</button>
```

The integration registers the Vite adapter and supplies automatic SSR styles
for pages and islands. An island written in React still imports
`stillwind/react` inside its React component.

## Next.js

Wrap your existing configuration in `next.config.mjs`:

```js
import { withStillwind } from "@stillwind/integration/next";

export default withStillwind({});
```

The wrapper adds the webpack and Turbopack loaders. Use `stillwind/react` in
components. Existing `use client` and `use server` directives are preserved.
Set the wrapper's `root` option when Next.js runs outside the application root.

## SvelteKit

Register Stillwind before SvelteKit in `vite.config.ts`:

```ts
import { stillwind } from "@stillwind/integration/vite";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [stillwind(), sveltekit()],
});
```

Add `src/hooks.server.ts` so development SSR includes styles with JavaScript
disabled:

```ts
export { handle } from "@stillwind/integration/sveltekit/handle";
```

If you already have a server handle, compose both handles with SvelteKit's
`sequence` helper. Use `stillwind/svelte` in components. Production uses the
emitted stylesheets.

## Other build tools

Stillwind also provides Nuxt, webpack, Rspack, Rsbuild, and standalone Turbopack
integrations. See the
[build integration reference](https://github.com/SegaraRai/stillwind/blob/main/docs/integrations.md)
for their configuration and CSS delivery contracts.

Continue with [The basics of style()](/docs/style/) to write component styles.
