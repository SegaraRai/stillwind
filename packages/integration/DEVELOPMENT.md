# Build integration development

See [repository setup](../../DEVELOPMENT.md) for workspace prerequisites. Build the
package and its dependencies from the repository root:

```sh
vp run @stillwind/integration#build
```

User configuration belongs in the [integration guide](../../docs/integrations.md).
The [CSS emission contract](../../docs/css-emission.md) defines producer selection,
linking, explicit values, and output ordering.

## Responsibilities and lifetimes

`@stillwind/config` owns configuration evaluation. `session.ts` constructs native
sessions and lazy TypeScript services and closes the checker. Adapters supply
resolution, reads, watches, diagnostics, and transport specifiers.

| Module                   | Responsibility                                                        |
| ------------------------ | --------------------------------------------------------------------- |
| `vite/`                  | Environment lifetimes, virtual modules, HMR, and single/split assets  |
| `astro/`                 | Astro configuration and middleware around the Vite adapter            |
| `nuxt/`                  | Nuxt Vite hooks and development SSR stylesheet handling               |
| `sveltekit/handle.ts`    | Development SSR page transformation                                   |
| `webpack/`               | Loader requests, producer metadata, and compilation linking           |
| `rspack/` and `rsbuild/` | Adapter registration using the webpack transport                      |
| `turbopack/`             | Producer discovery from configured entries in isolated loader workers |
| `next/`                  | Next route discovery and webpack/Turbopack loader registration        |

Vite's factory returns compiler, development, and build plugins sharing state for
one resolved configuration. Each environment owns its design system and semantic
state. Closing an environment leaves other environments available; server restarts
create a new project. The compiler plugin initializes and disposes environments,
the development plugin serves CSS, and the build plugin links assets to chunks.

Vite registers CSS projection imports while transforming their source documents.
Generated IDs refer to registered inputs and resources in that environment.
Direct projection URLs cannot register additional files.

webpack and Rspack record producer inputs in bundler metadata and reconstruct the
session from the completed graph, including cached and lazy modules. This allows
CSS emission when transforms are restored from a persistent cache.
Named loader functions disable module caching. SFC requests retain their configured
loader identity, while serializable options remain embedded for isolated workers.

Next and Turbopack reconstruct entry-reachable producers through loaders without
webpack compilation hooks or mutable worker-global state. File, resource, and
route-directory dependencies trigger reconstruction, including route additions
and removals.

## Internal loader hosts

`@stillwind/integration/internal/webpack-loader` exposes the producer loader for
hosts that configure loaders directly. It accepts compiler options from the loader
context and requires resolution and file-dependency operations. Query-qualified
dependencies also require `importModule` to capture transformed source without
executing application modules.

Without `StillwindPlugin`, a host can supply explicit `stillwind/module-css`
values with `automatic: false`. Automatic document CSS and stylesheet projections
require the plugin and a complete compilation graph.

## Verification

Run focused suites from the repository root:

```sh
vp run @stillwind/integration#test:vite
vp run @stillwind/integration#test:bundlers
vp run @stillwind/integration#test:frameworks
```

Vite fixtures cover browser targets and generated CSS. Regenerate them explicitly
with `vp run @stillwind/integration#generate:fixtures:vite` and review the diff.
Browser acceptance, host versions, profiles, and known limitations are maintained
in [e2e/DEVELOPMENT.md](../../e2e/DEVELOPMENT.md) and
[e2e/coverage.md](../../e2e/coverage.md). The `e2e/css` suite checks shared CSS
behavior; it does not establish every framework's streaming or HMR behavior.
