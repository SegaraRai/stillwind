# Build integrations

Start with the [Stillwind package guide](../packages/core/README.md) to create the
CSS entrypoint and `stillwind.config.ts`. Each adapter discovers that configuration
from its application root. Shared discovery, project selection, and inline overrides
are described in [Project configuration](standalone-analysis.md#shared-project-configuration).

## Vite

```ts
// vite.config.ts
import { stillwind } from "@stillwind/integration/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [stillwind(), react()],
});
```

Place Stillwind before the framework's JSX transform. Framework plugins that split
routes or server functions must run first. For TanStack Start, use
`[tanstackStart(), stillwind(), react()]`.

When `target` is omitted, Stillwind inherits the client's resolved `build.cssTarget`,
which Vite defaults to `build.target`. Inline Stillwind targets override project
targets, and project targets override inheritance. `target: []` disables inheritance.
Client and SSR compilation use the same browser target. Browser-version targets
such as `chrome111` are supported; disabled targets, JavaScript levels such as
`es2022`, and lists containing unsupported entries leave Stillwind targets unspecified.

Production CSS assets and explicit CSS values use Stillwind's Lightning CSS
minifier. `build.cssMinify: false` disables this step. Vite's minifier engine choice
continues to control Vite-owned CSS. Minification can lower syntax and add prefixes;
version components must be at most 255. See [browser targets](css-emission.md#browser-targets)
for fallback behavior.

For a custom development SSR document that must render with JavaScript disabled,
add a stylesheet link to `/__stillwind/style.css?environment=ssr` under Vite's public
base. The response includes every observed automatic producer in that environment.
Repeat `module` query parameters to select root-relative producer files, for example
`&module=src/routes/index.tsx&module=src/server-style.ts`. Each selection includes
its framework query modules and required style dependencies. The response is not cached.

## Astro

```ts
// astro.config.ts
import { stillwind } from "@stillwind/integration/astro";
import { defineConfig } from "astro/config";

export default defineConfig({
  integrations: [stillwind()],
});
```

Keep the application's other Astro integrations. Stillwind registers the Vite
adapter and supplies automatic SSR styles for pages and islands. The optional
Astro peer is required only when using this entrypoint.

## Nuxt

```ts
// nuxt.config.ts
import { stillwind } from "@stillwind/integration/nuxt";

export default defineNuxtConfig({
  modules: [stillwind()],
});
```

The module registers the Vite adapter and development SSR stylesheet handling.
Do not also register the same project through a separate Vite Stillwind plugin.

## SvelteKit

Register Stillwind before SvelteKit:

```ts
// vite.config.ts
import { stillwind } from "@stillwind/integration/vite";
import { sveltekit } from "@sveltejs/kit/vite";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [stillwind(), sveltekit()],
});
```

Add the public handle so development SSR includes styles without client JavaScript:

```ts
// src/hooks.server.ts
export { handle } from "@stillwind/integration/sveltekit/handle";
```

If the application already has a handle, compose it with Stillwind's `handle`
using SvelteKit's `sequence` from `@sveltejs/kit/hooks`. Production uses Vite's
emitted stylesheets.

## Next.js

```js
// next.config.mjs
import { withStillwind } from "@stillwind/integration/next";

export default withStillwind({});
```

Pass the application's existing configuration as the first argument and Stillwind
overrides as the optional second argument. The wrapper retains existing rules and
webpack hooks and adds loaders for webpack and Turbopack. It preserves
`use client` and `use server` directives.

The stylesheet includes JavaScript/TypeScript routes under `app`, `src/app`,
`pages`, or `src/pages` and their statically named local imports, including lazy
routes. This preserves CSS order across navigation. It does not perform bundler
tree-shaking or enumerate runtime-computed imports. Custom route syntax that needs
another loader, such as MDX, is outside this transport.

Set `root` when Next runs from a directory other than the application root.
Inline loader options must be serializable; declare naming callbacks in the
configuration file instead. Explicit `stillwind/module-css` values remain scoped
to their producer and composition dependencies.

## webpack and Rspack

```js
// rspack.config.mjs
import { StillwindPlugin } from "@stillwind/integration/rspack";

export default {
  plugins: [new StillwindPlugin()],
  module: {
    rules: [{ test: /\.css$/, type: "css" }],
  },
};
```

For webpack, import `StillwindPlugin` from `@stillwind/integration/webpack` and keep
the application's CSS loader pipeline. Next's wrapper supplies its own loaders
and CSS pipeline.

In webpack, place `StillwindPlugin` before `VueLoaderPlugin`. Vue's plugin clones
loader rules during setup, so Stillwind's rules must already be installed.

Set `automatic: false` when a library supplies its styles through explicit
`stillwind/module-css` values. Producer compilation remains enabled, while ordinary
producers stop importing document CSS.

## Rsbuild

```ts
// rsbuild.config.ts
import { defineConfig } from "@rsbuild/core";
import { pluginStillwind } from "@stillwind/integration/rsbuild";

export default defineConfig({
  plugins: [pluginStillwind()],
});
```

Retain the application's framework plugin. Stillwind runs after framework
pre-loaders split routes and server functions and before SWC lowers JSX.
A route can retain a server-only producer's styles with
`import "./server-style.ts?stillwind.css"`; the import does not ship its JavaScript.

## Standalone Turbopack

```ts
import { stillwindRules } from "@stillwind/integration/turbopack";

const rules = stillwindRules({
  entries: ["./src/main.tsx", "./src/other-entry.tsx"],
});
```

Add these rules to the host's Turbopack configuration while retaining existing rules.
This entrypoint does not depend on Next or discover routes. List every application
entry, relative to `root` or as an absolute path. The default root is the process
working directory. Entry globs are not expanded.

The loader follows statically named local JS/TS imports, including lazy imports,
and emits one ordered stylesheet. Runtime-computed imports and custom syntax such
as MDX are outside this transport. The host must support webpack-style loaders
with `getResolve`, file dependencies, and CSS output through `as: "*.css"`.
Inline options must be serializable. `mode` can select `development` or `release`;
otherwise it follows `NODE_ENV`.

## CSS delivery

Vite and its wrappers default to one stylesheet. `emission: "split"` emits reusable
partitions and `stillwind-manifest.json`. Custom SSR documents use
`stylesheetLinks` from `stillwind/css` to select ordered links. See
[Application builds and SSR](css-emission.md#application-builds-and-ssr) for manifest
handling and the requirement to share naming settings between client and server.

When a framework removes a server-only dependency from the client graph, retain
its styles from the route that renders them:

```ts
import { serverProps } from "../server.server.js";
import "../server.server.js?stillwind.css";
```

The CSS projection compiles the producer and its semantic dependencies without
evaluating or bundling the server JavaScript.

Browser-facing Vite CSS maps omit source contents. Known client scripts and CSS
inside the project use relative `stillwind-source:` names. Server sources, component
documents, and unrecognized upstream sources use opaque names. These names are
not URLs from which the browser can retrieve original files.

Use explicit `stillwind/module-css` values for manual placement or Shadow DOM.
See [Web Components and explicit values](css-emission.md#web-components-and-explicit-values)
for adoption and shared stylesheet ownership.

For tested host versions and rendering modes, see [Coverage and limitations](../e2e/coverage.md).
