# Stillwind build integrations

The JavaScript packages publish ESM and generated declarations from `dist/`.
Their `build` tasks use `vp pack`; the native package builds its Node-API binary
before packing the TypeScript binding loader.

Build the integration and its package dependencies:

```sh
vp run --filter @stillwind/integration build
```

The application needs `stillwind` and `@stillwind/integration` as dependencies. Its
CSS entry can use ordinary package exports, for example `@import "stillwind";`.

Application tasks must depend on the integration build, including development:

```ts
run: {
  tasks: {
    dev: { command: "vp dev", dependsOn: ["@stillwind/integration#build"] },
    build: { command: "vp build", dependsOn: ["@stillwind/integration#build"] },
  },
}
```

## Responsibilities

`options.ts` owns the shared compiler configuration and vocabulary generation.
`session.ts` owns native session construction and lazy TypeScript facts, including
closing the checker. Each adapter supplies resolution, reads, watches, diagnostics
and transport specifiers to that session.

`vite/` owns environment lifetime, virtual modules, HMR and single/split assets.
`webpack/` owns loader requests, cached producer metadata and compilation linking;
Rspack and Rsbuild reuse those modules. A completed webpack
compilation reconstructs its session from producer metadata, so CSS emission also
works when module transforms are restored from cache. Vite and its Astro wrapper expose `emission`.
`astro/` wraps that Vite adapter through Astro's configuration hook.
`turbopack/` reconstructs entry-reachable producers in loader workers and
hands one ordered stylesheet to the host's CSS pipeline. `next/` supplies Next's
route discovery and registers those loaders for webpack and Turbopack.

All adapters share `css`/`theme`, root-relative identity, compiler flags, naming and
vocabulary settings. Named loader functions disable module caching and SFC requests
retain their configured loader option identity; serializable options remain embedded
in requests for isolated workers. Turbopack's loader configuration must be serializable.

## Vite

```ts
import { stillwind } from "@stillwind/integration/vite";
```

See the [Vite integration](src/vite/README.md) for plugin configuration, CSS emission
and framework placement.

## Astro

```ts
import { defineConfig } from "astro/config";
import { stillwind } from "@stillwind/integration/astro";

export default defineConfig({
  integrations: [stillwind({ css: "./src/stillwind.css" })],
});
```

The `astro:config:setup` hook adds the public Vite plugin with the same options,
including `emission`. Keep the application's other Astro integrations. The optional
Astro peer dependency is required only for this subpath.

## Turbopack

```ts
import { stillwindRules } from "@stillwind/integration/turbopack";

const rules = stillwindRules({
  css: "./src/stillwind.css",
  entries: ["./src/main.tsx", "./src/other-entry.tsx"],
});
```

Add these rules to the host's Turbopack rules configuration, retaining existing
rules. This entrypoint does not depend on Next or discover framework routes.
`entries` must list every application entry file, relative to `root` (default:
the process working directory) or absolute. The loader follows their statically
named local JS/TS imports, including lazy imports, and emits one ordered stylesheet
for that graph. Add independent entries to this list when the application grows.
Runtime-computed imports and custom source syntaxes such as MDX are outside this
transport. Entry globs are not expanded.

The host must support webpack-style loaders with `getResolve`, file dependencies,
and CSS output via `as: "*.css"`. Next's automatic route discovery additionally
uses directory dependencies. Compiler options are shared with other adapters,
except naming callbacks cannot be serialized. `mode` can explicitly select
`development` or `release`; otherwise it follows `NODE_ENV`.

The `standalone` profile in `e2e/frameworks/next16-react19` exercises these public rules
directly, without `withStillwind`, using Next as the Turbopack test host. It covers
development updates and cold/warm production. Other Turbopack hosts are not covered
by that fixture.

## Next.js

```js
import { withStillwind } from "@stillwind/integration/next";
export default withStillwind({}, { css: "./src/stillwind.css" });
```

This installs Next-specific loaders for both webpack and Turbopack. Existing
configuration and webpack hooks are retained. Synthetic imports follow the source body so that
`use client` and `use server` directives remain active.

Next's stylesheet loader starts at the JavaScript/TypeScript route components
in `app`, `src/app`, `pages`, or `src/pages`, follows statically named local module
imports, and compiles their producers into one native stylesheet plan. It includes
all routes, including lazy routes, to preserve CSS order across navigation. It
does not perform bundler tree-shaking or enumerate runtime-computed import paths.
Use JavaScript/TypeScript route entries; custom syntax requiring another loader,
such as MDX, is outside this route-graph transport.

Source and CSS reads, imported resources, and route directories are registered as
loader dependencies. Each rebuild reconstructs the graph, including route additions
and removals, without mutable worker-global state or generated CSS files in the app.
Explicit `stillwind/module-css` values remain scoped to their producer and its
composition dependencies. The application root defaults to the Next process's
working directory; set `root` when invoking Next from a different directory.
Next loader options must be serializable, so naming callbacks are rejected.

The implementation uses `getResolve`, file/context dependencies, and a CSS-output
loader rather than webpack compilation hooks. See the official
[Turbopack loader API and module types](https://nextjs.org/docs/app/api-reference/config/next-config-js/turbopack).

## Rspack

```js
import { stillwindRules, StillwindPlugin } from "@stillwind/integration/rspack";
export default {
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
server functions, editing, and cold/warm production. The fixture declares its own
framework dependencies, and the frozen lockfile is at the workspace root.
Run `vp run e2e` in `e2e/frameworks/tanstackstart1-react19-server-function` with the
`rsbuild` profile to reproduce on port 43179.

## Transport and verification

Webpack/Rspack automatic producers record their inputs in bundler metadata. `StillwindPlugin`
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

Run the matrix from the repository root after `vp i --frozen-lockfile`:

```sh
vp run verify:e2e
```

The probes use the real compiler and Chrome. The package test suite checks current
webpack and Rspack builds for React, Preact, Solid, Vue and Svelte. The broader
browser probes are separate acceptance checks: Next checks dev CSS edits, macro
removal/restoration, hydration, navigation, prefetch and cold/warm production
output. Rspack/Rsbuild check native or extracted CSS chunks, relative images,
persistent-cache reuse, CSS changes, unreachable producers and macro removal.
The Next probe also checks cross-module composition edits and Turbopack route
addition/removal. These probes do not claim expression-level dead-code elimination.
The current Next acceptance fixture pins `16.4.0-canary.25` and exercises both
Turbopack and webpack; other Next versions are not covered by that run.

The [historical acceptance matrix](../../docs/module-graph-implementation.md)
records framework and source-map results for the earlier transport.
