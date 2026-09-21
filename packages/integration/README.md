# Stillwind build integrations

Build the local Node runtime before starting an application:

```sh
vp run --filter @stillwind/integration build
```

The application needs `stillwind` and `@stillwind/integration` as dependencies. Its
CSS entry can use ordinary package exports, for example `@import "stillwind";`.

## Vite

```ts
import { stillwind } from "@stillwind/integration/vite";
```

See the [Vite integration](src/vite/README.md) for plugin configuration, CSS emission
and framework placement.

## Next.js

```js
const { withStillwind } = require("@stillwind/integration/next");
module.exports = withStillwind({}, { css: "./src/stillwind.css" });
```

This installs webpack graph linking and loader rules for both bundlers. Turbopack's
loader-only API supports explicit `stillwind/module-css` values; automatic document
CSS requires the webpack integration. Existing configuration and webpack hooks are
retained. Synthetic imports follow the source body so that
`use client` and `use server` directives remain active.

## Rspack

```js
const { stillwindRules, StillwindPlugin } = require("@stillwind/integration/rspack");
module.exports = {
  plugins: [new StillwindPlugin()],
  module: {
    rules: [...stillwindRules({ css: "./src/stillwind.css" }), { test: /\.css$/, type: "css" }],
  },
};
```

For webpack, import the plugin and rules from `@stillwind/integration/webpack` alongside the application's existing CSS loader
pipeline. Next's webpack integration supplies its own CSS pipeline.

## Rsbuild

```js
import { defineConfig } from "@rsbuild/core";
import { pluginStillwind } from "@stillwind/integration/rsbuild";
export default defineConfig({
  plugins: [pluginStillwind({ css: "./src/stillwind.css" })],
});
```

Retain the application's framework plugin when adding Stillwind.

The Rsbuild adapter runs after framework pre-loaders split routes and server
functions, and before SWC lowers JSX. A server-only producer can be connected to
its route with `import './server-style.ts?stillwind.css'`; this import compiles
only its stylesheet and does not ship its JavaScript.

The TanStack Start/Rsbuild experiment covers SSR without JavaScript, lazy route CSS,
server functions, editing, and cold/warm production. On Windows it pins a patch for `@tanstack/start-plugin-core`
1.171.40: its Rsbuild manifest must normalize backslashes before matching
route paths. The patch and frozen lockfile are in `experiments/module-graph`.
Run `vp run verify:tanstack-rsbuild` there to reproduce on port 43179.

## Transport and verification

Automatic producers record their inputs in bundler metadata. `StillwindPlugin`
links the completed compilation graph, including lazy and cached modules, and
rebuilds one shared CSS module for the host's extraction or HMR pipeline. The plugin
must be installed before compilation begins, including on Rspack.

Explicit CSS values import immutable fragment modules. Common payloads share bytes.
Each value retains its ordered rules and required producer styles.
Set `automatic: false` in the adapter options when a library supplies all CSS
through explicit values, including dependencies on shared style modules.
CSS inputs, semantic inputs, resource
files, the host implementation and the native binary are build dependencies.
Class identity defaults to the module path relative to the application root;
set `root` explicitly when different build environments use different roots.

Run these from `experiments/module-graph` after `vp i --frozen-lockfile`:

```sh
vp run verify:adapters
vp run verify:webpack
vp run verify:rspack
vp run verify:rsbuild
```

The probes use the real compiler and Chrome. The package test suite checks current
webpack and Rspack builds for React, Preact, Solid, Vue and Svelte. The broader
experiment probes are separate acceptance checks: Next checks dev CSS edits, macro
removal/restoration, hydration, navigation, prefetch and cold/warm production
output. Rspack/Rsbuild check native or extracted CSS chunks, relative images,
persistent-cache reuse, CSS changes, unreachable producers and macro removal.
These probes do not claim expression-level dead-code elimination or complete
graph enumeration from Turbopack loader callbacks.

The [historical acceptance matrix](../../docs/module-graph-implementation.md)
records framework and source-map results for the earlier transport. Current
Turbopack integration supports explicit CSS values only.
