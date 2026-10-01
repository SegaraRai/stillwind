# Framework browser acceptance

This package belongs to the repository's root pnpm workspace and tests the real Stillwind compiler with installed Google Chrome. The root lockfile pins the framework matrix. The unplugin patch in `patches/` composes source maps in its Rspack transform loader.

Install once from the repository root with `vp i --frozen-lockfile`. From this directory, run `vp run test` for source-map helper tests and `vp run typecheck` for the TypeScript test entry points. Run `vp run browser` for the complete browser matrix, or select one scenario with `vp run browser:<name>`:

| Scenario                                                          | Contract                                                                                             |
| ----------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `composition`                                                     | Base-style dependencies, cascade ordering, edits, SSR and hydration                                  |
| `emission`                                                        | Split/single CSS, original/minified output, stylesheet order, SSR and Web Components with Shadow DOM |
| `conditional`                                                     | Nested conditional properties, SSR and hydration                                                     |
| `runtime-if`                                                      | CSS computed-value conditions in both compiler output modes                                          |
| `animation`                                                       | Keyframes and reduced-motion branches                                                                |
| `view-transition`                                                 | Literal/runtime keys, conditions and unnamed transitions                                             |
| `next-turbopack`, `next-standalone`, `next-webpack`               | RSC/client styles, navigation, dependency edits and cold/warm builds                                 |
| `rspack`, `rsbuild`                                               | Assets, CSS, dependency invalidation and persistent caches                                           |
| `react-router`, `solidstart`, `tanstack-vite`, `tanstack-rsbuild` | Framework SSR, hydration, lazy routes, edits and cold/warm builds                                    |

The repository entry point is `vp run verify:e2e`. Ordinary `vp run verify` does not require Chrome or run this matrix.

Vitest runs `tests/browser.test.ts` sequentially. Each scenario starts a separate Node process because framework CLIs, server lifecycles and their process arguments require isolation. The `.mjs` files in `scripts/` are those process runners. The test entry point checks their exit status with `expect`, retains failure output, terminates unfinished runner processes and removes the fixtures created by its run. Generated applications and caches live in ignored `output/`; reports and framework logs live in ignored `results/`.

The runners use ports 43171–43181. Run one browser matrix at a time. Vite dependency edits may reload the page; these checks do not guarantee that HMR preserves application state. The source-map checks cover the generated fixture modules, rather than every framework template or deployment environment.

React Router, SolidStart and TanStack Vite use split emission and select development SSR styles with an explicit stylesheet link. Their lazy-route checks require CSS to stay absent until navigation. TanStack Rsbuild uses one document stylesheet, which includes lazy-route CSS before navigation. The TanStack runner defines a server function inside the route and references it from both the loader and component, exercising generated shared modules. It checks the RPC response, server-only content and CSS projection. Both TanStack backends check CSS source coordinates after the framework transforms the route.
