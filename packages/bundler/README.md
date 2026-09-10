# Stillwind bundler adapters

Build the local Node runtime before starting an application:

```sh
vp run --filter @stillwind/bundler build
```

The application needs `stillwind` and `@stillwind/bundler` as dependencies. Its
CSS entry can use ordinary package exports, for example `@import "stillwind";`.

## Next.js

```js
const { withStillwind } = require("@stillwind/bundler");
module.exports = withStillwind({}, { css: "./src/stillwind.css" });
```

This installs both Turbopack and webpack rules. Existing Next configuration and
webpack hooks are retained. Synthetic imports follow the source body so that
`use client` and `use server` directives remain active.

## Rspack

```js
const { stillwindRules } = require("@stillwind/bundler");
module.exports = {
  module: {
    rules: [...stillwindRules({ css: "./src/stillwind.css" }), { test: /\.css$/, type: "css" }],
  },
};
```

For webpack, use the same rules alongside the application's existing CSS loader
pipeline. Next's webpack integration supplies its own CSS pipeline.

## Rsbuild

```js
import { defineConfig } from "@rsbuild/core";
import { pluginStillwind } from "@stillwind/bundler";
export default defineConfig({
  plugins: [pluginStillwind({ css: "./src/stillwind.css" })],
});
```

Retain the application's framework plugin when adding Stillwind.

## Transport and verification

Each transform creates an isolated native session. CSS query payloads contain
the document stylesheet and selected module contribution; another worker can
reconstruct them without a shared map. CSS inputs, semantic inputs, resource
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

The probes use the real compiler and Chrome. Next checks dev CSS edits, macro
removal/restoration, hydration, navigation, prefetch and cold/warm production
output. Rspack/Rsbuild check native or extracted CSS chunks, relative images,
persistent-cache reuse, CSS changes, unreachable producers and macro removal.
These probes do not claim expression-level dead-code elimination or complete
graph enumeration from Turbopack loader callbacks.
