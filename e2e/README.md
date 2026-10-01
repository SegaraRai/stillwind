# Browser acceptance

Each application under frameworks/ and bundlers/ is a private package in the root pnpm workspace. Its package.json declares its own framework versions and plugins. Application files stay in the framework's normal layout. The runner edits a disposable copy under that package's .e2e/ directory, where dependency resolution still uses the package's node_modules.

Install from the repository root with `vp i --frozen-lockfile`. Installed Google Chrome is required for browser acceptance. Run `vp run verify:e2e` for the complete matrix. Ordinary `vp run check` checks the harness and fixture TypeScript, and `vp run test` tests harness helpers without opening a browser.

To run one application, enter its directory and run `vp run e2e`. Its `vp run dev`, `vp run build`, and `vp run serve` tasks use its own framework CLI or bundler API. Set `STILLWIND_E2E_PROFILE` to one of its declared profiles to select a configuration. Set `STILLWIND_E2E_DEV_ONLY=1` to omit production checks in the common runner. Specialized process tests still run their own production contracts.

The matrix runs at most three packages concurrently. `STILLWIND_E2E_CONCURRENCY` accepts integers from 1 to 8. Common tests allocate TCP ports dynamically. Specialized tests retain distinct ports in the 43171–43181 range, so run one matrix at a time. After building the integration package, `vp exec node e2e/harness/src/matrix.ts vite8-react19 next16-react19` selects packages by directory name.

## Applications

| Host             | Component combinations                                                   | Configuration profiles or additional applications                                     |
| ---------------- | ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------- |
| Vite 8           | React 19, Preact 11, Vue 3 SFC, Vue 3 JSX, Solid 1, Solid 2 RC, Svelte 5 | single and split CSS; detailed React SSR scenarios within vite8-react19               |
| webpack 5        | React 19, Preact 11, Solid 1, Vue 3, Svelte 5                            | one package per combination                                                           |
| Rspack 2         | React 19, Preact 11, Solid 1, Vue 3, Svelte 5                            | one package per combination; plain asset/cache application                            |
| Rsbuild 2        | React 19, Preact 11, Solid 1, Vue 3, Svelte 5                            | one package per combination; plain asset/cache application                            |
| Nuxt 4           | Vue 3                                                                    | SSR, single and split CSS                                                             |
| SvelteKit 2      | Svelte 5                                                                 | SSR with the Node adapter, single and split CSS                                       |
| Astro 7          | plain; React 19 + Svelte 5 + Vue 3; Preact 11 + Solid 1                  | static prerendering and hydrated islands, single and split CSS                        |
| Next 16          | React 19                                                                 | Turbopack, webpack, and standalone Turbopack rules                                    |
| React Router 8   | React 19                                                                 | SSR, server-only projection and lazy routes                                           |
| SolidStart 2     | Solid 1                                                                  | SSR and lazy routes                                                                   |
| TanStack Start 1 | React 19 and Solid 1                                                     | single and split CSS; separate React server-function application for Vite and Rsbuild |

Suffixes identify applications that require different source code or dependencies. Profiles select configurations within the same application. Vue JSX and Solid 2 RC need separate packages because their compiler plugins differ. Framework versions that cannot use the same official plugin are not substituted through aliases. The root lockfile records exact resolutions.

The matrix exercises the selected current versions of the component formats and hosts exposed by Stillwind. [Coverage and limitations](coverage.md) distinguishes these checks from other supported versions and rendering modes. Metaframeworks use their supported component framework; the table does not imply a cross product between unrelated metaframeworks and component plugins. Astro's official integrations cover all five supported island frameworks across two applications.

## Manifests and contracts

A common fixture.ts declares its driver, profiles, package versions, page selectors, expected CSS, counters, SSR or prerendering capability, and hydration capability. Each edit names a file and one literal replacement. The runner requires that literal to occur exactly once before writing. It restores the original bytes after the assertion and removes its work directory after the run. A capability that does not apply records a reason.

The same runner checks initial computed CSS, HTML with JavaScript disabled, hydration through interactive counters, and each declared edit. It checks document identity separately from component state. An edit that declares document preservation fails if a full reload occurs. State may remain unchanged or reset when the framework remounts components. Astro page edits declare document reloads because its static page contains server-rendered markup. Cold production builds use a fresh copy; warm builds reuse that copy and its build caches.

Next, React Router, and SolidStart use the common rendering and CSS-edit contracts alongside their specialized tests. Their specialized scripts retain prefetch, lazy CSS, source-map, server-only projection, macro removal, dependency invalidation and build checks. Next also checks Flight response types and excludes a server implementation sentinel from client JavaScript, CSS and source maps. Each common application includes composition through several modules, a diamond of dependencies, inherited hover rules, and nested condition branches. Interactive frameworks cycle through every branch. Astro without islands checks static branch output. vite8-react19 also contains detailed composition and conditional SSR scenarios; these scenarios are tests inside the package. The TanStack server-function application has specialized scripts for its RPC endpoint. Their manifests list those contracts. Source-map tests retain narrowly declared upstream mismatches as separate evidence; they do not treat a mismatch as successful map support.

Nuxt creates a plugin instance for each Vite server. Its configuration routes a development stylesheet request to the SSR server, and disposes that stylesheet after hydration. Astro embeds explicit module CSS values from its page and island components in the prerendered HTML. Island applications use hashed chunk names because explicit CSS fragment identifiers can exceed Windows filename limits. Its CSS entry loads the theme and builtin registry without a preflight reset.

## CSS behavior and artifacts

Framework-independent Stillwind CSS behavior lives in css/: emission order, single/split output, minification, Shadow DOM, computed-value conditions, animation and view transitions. Composition and condition branches are checked inside the normal framework applications. Plain bundler fixtures live under bundlers/ because they test asset emission and persistent caches rather than framework components.

The shared harness owns workspace copies, processes, browser assertions and source-map checks. Common runs write report.json with actual dependency versions and completed contracts, HTML from SSR checks, browser traces, server logs and failure screenshots into each package's results/ directory. Acceptance scripts use TypeScript and run directly in Node. Each package includes its scripts in its TypeScript check. Specialized runs retain their reports and process output in a unique results/ directory. The matrix writes exit status and package logs under e2e/results/. All application outputs and results are ignored by Git.

The root unplugin patch composes source maps in its Rspack transform loader. Its location and version are declared in pnpm-workspace.yaml.
