# Stillwind for Vite

Place `stillwind({ css: './root.css' })` before a framework's JSX transform.
The CSS entry resolves normal package exports, for example `@import 'stillwind';`.
Generated module CSS uses self-contained query imports. The adapter owns separate
design systems and semantic state for each Vite environment.

## Host naming

`naming` supplies the host's own generators, one optional function per identifier
family; `namingRevision` hashes them so cached artifacts are not reused across a
change. A generator receives its family's context and the whole request hash, and
returns the name to write or `undefined` to keep the compiler's name. Generators
must be pure. A name a generator writes is checked against the document's authored
names, including the stylesheets a producer imports.

The underlying CSS resources have physical file identities so framework CSS
discovery can include them in server-rendered HTML. Module carriers preserve the
entry's layer ordering even when a framework emits the route CSS before shared
base CSS. Development imports have a revision to prevent an earlier HMR prune
from removing the CSS of a restored macro; class identity stays unchanged.

## Server-only styles

When a framework removes a server-only JavaScript dependency from its client
graph, add a CSS-only edge from the corresponding route:

```tsx
import { serverProps } from "../server.server.js";
import "../server.server.js?stillwind.css";

export function loader() {
  return { props: serverProps };
}
```

The projection compiles the named module and its demanded semantic dependencies
without evaluating JavaScript. It imports the resulting module CSS and shared
base CSS through Vite. It does not copy the source JavaScript into the client
bundle or scan the application's runtime graph. The normal server import remains
responsible for evaluating server code. Project only the producers whose styles
the route renders; this preserves route CSS splitting. Relative `.server.js`
file imports are covered by the React Router acceptance test. A framework may
apply additional restrictions to other server-only directory conventions.

CSS and source dependency edits invalidate these projections. The request's
source path is carried in its ID, so a clean process can rebuild the projection
without a module registry populated by a previous server transform.

## Verified framework

React Router 8.3.1 Framework Mode uses the normal `reactRouter()` plugin with
`stillwind()` first. The executable fixture in
`experiments/module-graph/scripts/verify-react-router.mjs` checks SSR with
JavaScript disabled, hydration, client navigation, lazy CSS, repeated macro
removal/restoration, CSS entry edits, server-only projection edits, and separate
cold/warm production builds. It also verifies generated JavaScript source maps
back to the macro's line and column and excludes server source from client
JavaScript and maps. CSS source-map provenance is a separate capability.

Reproduce from the repository root:

```sh
vp run --filter @stillwind/vite test
cd experiments/module-graph
vp i --frozen-lockfile
vp run verify:react-router
```

The runner uses installed Chrome and port 43176. Development may reload the page;
preserving component state during all framework HMR updates is not asserted.

SolidStart 2.0.4 uses `solidStart()` and `nitro()` after `stillwind()`. Import the
macro from `stillwind/solid` to emit Solid's `class` prop. Reactive expressions
stay inside JSX spreads for Solid's transform to track them. Run
`vp run verify:solidstart` in `experiments/module-graph` (port 43177) for
JavaScript-disabled SSR, hydration, signal updates, lazy client navigation,
CSS edits, macro removal/restoration, and cold/warm production builds.
The local linked-package fixture explicitly allows the repository in Vite's
`server.fs.allow`. Carriers end in `.css`, including their immutable payload and
development revision, so framework CSS discovery and `?inline` reads work.

For TanStack Start, put `stillwind()` **after** `tanstackStart()` and before the
React plugin. Route splitting must run before Stillwind assigns CSS ownership;
otherwise route CSS becomes eager. Server-function-only producers use the
explicit `?stillwind.css` projection described above. Run
`vp run verify:tanstack-vite` in `experiments/module-graph` (port 43178) to verify
SSR, hydration, server-function calls, lazy CSS, edits, and cold/warm builds.
