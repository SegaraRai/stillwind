# Stillwind for Vite

Place `stillwind({ css: './root.css' })` before a framework's JSX transform.
The CSS entry resolves normal package exports, for example `@import 'stillwind';`.
The adapter owns separate design systems and semantic state for each Vite environment.

## CSS emission

Development publishes versioned CSS plans to one retained style element. The runtime
activates producer dependencies in reference order and retracts pruned contributions.
CSS-only edits whose generated JavaScript stays unchanged update styles without
invalidating producer JavaScript. Framework HMR still handles JavaScript changes.

For development SSR with JavaScript disabled, include a stylesheet link to
`/__stillwind/style.css?environment=ssr` in the document head, under Vite's public
base. This endpoint returns the selected environment's observed CSS without caching.
Repeat `module` parameters to select root-relative producer files, for example
`&module=src/routes/index.tsx&module=src/server-style.ts`. Each file includes its
framework-generated query modules and required style dependencies. Without these
parameters, the response includes every observed automatic producer, including
routes a framework scans before navigation.

Production defaults to one optimized stylesheet after the full graph, including lazy
modules, is known. Set `emission: "split"` for reusable producer partitions and an
ordered `stillwind-manifest.json`. SSR selects links with `stylesheetLinks` from
`stillwind/css`; the client uses the same manifest when adding selections.
Split CSS assets also participate in Vite's chunk metadata so framework SSR asset
manifests can include them in document links.

```ts
stillwind({ css: "./root.css", emission: "split" });
```

`import { css, parts } from 'stillwind/module-css'` returns the module's CSS and
required producer styles as text and shared fragments, without automatically applying that
producer's CSS. `styleSheets(parts, document)` from `stillwind/css` creates shared
constructed sheets for Shadow DOM adoption.
Set `automatic: false` for a Web Components library to suppress document loading
from its shared style modules as well.

See [CSS emission](../../../../docs/css-emission.md) for ordering, optimization,
source maps, shared payloads and placement limits.

## Host naming

`naming` supplies optional name generators by identifier family; `namingRevision`
invalidates cached artifacts after generator changes. Generators must be pure.
Generated names are checked against authored and generated names in their namespace.
Use the same identifier configuration for client and server builds.

## Server-only styles

When a framework removes a server-only dependency from its client graph, add an
explicit CSS projection from the route that renders its styles:

```tsx
import { serverProps } from "../server.server.js";
import "../server.server.js?stillwind.css";
```

The projection compiles that producer and its semantic dependencies without evaluating
or bundling the server JavaScript. It participates in the document emission plan.
CSS maps retain source coordinates and omit source contents when a projection is
present. The source path in its ID permits reconstruction in a fresh process.

Framework route splitting must precede Stillwind's transform when it changes producer
ownership. In TanStack Start, place `stillwind()` after `tanstackStart()` and before
the React plugin.

## Verification

`vp run @stillwind/integration#test:vite` checks compilation, environment isolation, resources,
source maps and explicit values. `vp run @stillwind/integration#test:frameworks` builds with
the official Vue, Svelte and Astro integrations.

In `e2e/css`, run `vp run test:e2e` with installed Chrome.
It compares computed styles across development, single/split builds, minification,
reverse lazy-route loads, JavaScript-disabled SSR and Shadow DOM. It also checks
CSS-only HMR, module addition/deletion and shared stylesheet instances.

Framework-specific packages under `e2e/frameworks` are separate acceptance probes;
passing the emission probe does not establish every framework's streaming or HMR behavior.
