# Browser acceptance

Each application under frameworks/ and bundlers/ is a private package in the root pnpm workspace. Its package.json declares its own framework versions and plugins. Application files stay in the framework's normal layout. The runner edits a disposable copy under that package's .e2e/ directory, where dependency resolution still uses the package's node_modules.

Install from the repository root with `vp i --frozen-lockfile`. Installed Google Chrome is required for browser acceptance. Run `vp run test:e2e` for the complete matrix. Ordinary `vp run check` checks the harness and fixture TypeScript, and `vp run test` tests harness helpers without opening a browser.

To run one application, enter its directory and run `vp run test:e2e`. Its `vp run dev`, `vp run build`, and `vp run serve` tasks use its own framework CLI or bundler API. Set `STILLWIND_E2E_PROFILE` to one of its declared profiles to select a configuration. Set `STILLWIND_E2E_DEV_ONLY=1` to omit production checks in the common runner. Specialized suites still run their own production contracts.

For a named common contract, enter a fixture package and run `vp test --run --config acceptance.config.ts -t "reflects component edits"`. The verbose reporter shows the fixture, configuration profile, development or production phase, and contract name.

SvelteKit 3 requires `vp run generate:types` before direct Vitest commands so dependency optimization can resolve `$app/tsconfig`. Its normal tasks declare this prerequisite.

The matrix runs at most three packages concurrently. `STILLWIND_E2E_CONCURRENCY` accepts integers from 1 to 8. Common and specialized tests allocate TCP ports dynamically. `vp run @stillwind-internal/e2e-harness#test:e2e vite8-react19 next16-react19` builds the required artifacts and selects packages by directory name. Interactive fixture tasks use development ports in the 5200 range and preview ports in the 5300 range. `serve` builds first, and TanStack development, build, and typechecking depend on `generate:routes`.

## Applications

| Host             | Component combinations                                                   | Configuration profiles or additional applications                                     |
| ---------------- | ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------- |
| Vite 8           | React 19, Preact 11, Vue 3 SFC, Vue 3 JSX, Solid 1, Solid 2 RC, Svelte 5 | single and split CSS; detailed React SSR scenarios within vite8-react19               |
| webpack 5        | React 19, Preact 11, Solid 1, Vue 3, Svelte 5                            | one package per combination                                                           |
| Rspack 2         | React 19, Preact 11, Solid 1, Vue 3, Svelte 5                            | one package per combination; plain asset/cache application                            |
| Rsbuild 2        | React 19, Preact 11, Solid 1, Vue 3, Svelte 5                            | one package per combination; plain asset/cache application                            |
| Nuxt 4           | Vue 3                                                                    | SSR, single and split CSS                                                             |
| SvelteKit 2      | Svelte 5                                                                 | SSR with the Node adapter, single and split CSS                                       |
| SvelteKit 3      | Svelte 5                                                                 | SSR with the Node adapter, single and split CSS                                       |
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

Next, React Router, and SolidStart use the common rendering and CSS-edit contracts alongside their specialized tests. Their specialized cases retain prefetch, lazy CSS, source-map, server-only projection, macro removal, dependency invalidation and build checks. Next also checks Flight response types and excludes a server implementation sentinel from client JavaScript, CSS and source maps. Each common application includes composition through several modules, a diamond of dependencies, inherited hover rules, and nested condition branches. Interactive frameworks cycle through every branch. Astro without islands checks static branch output. vite8-react19 also contains detailed composition and conditional SSR scenarios; these scenarios are tests inside the package. The TanStack server-function application has specialized cases for its RPC endpoint. Their manifests list those contracts. Source-map tests retain narrowly declared upstream mismatches as separate evidence; they do not treat a mismatch as successful map support.

Nuxt uses `@stillwind/integration/nuxt`, which registers its Vite plugins and development SSR stylesheet. SvelteKit uses the Vite plugin and exports the public `@stillwind/integration/sveltekit/handle` from its server hooks. Astro's integration adds middleware that supplies automatic SSR styles for pages and islands. Fixture components do not export CSS for this transport.

SSR tests also make fresh requests after edits with JavaScript disabled and compare generated classes before and after hydration. Existing documents without JavaScript have no live-update contract. Production invalidation cases rebuild after each declared edit. Vue fixtures inspect production source maps in minified and unminified profiles. Rsbuild also checks development maps for template and script calls; documented map gaps use `test.fails` with the original position assertion intact. Reports distinguish `expected-failure` from supported passing cases.

See [coverage.md](./coverage.md) for the ownership of framework, application and unsupported behavior, and the remaining implementation gaps.

## CSS behavior and artifacts

Framework-independent Stillwind CSS behavior lives in css/: emission order, single/split output, minification, Shadow DOM, computed-value conditions, animation and view transitions. Composition and condition branches are checked inside the normal framework applications. Plain bundler fixtures live under bundlers/ because they test asset emission and persistent caches rather than framework components.

Case declarations live in each package's tests/ directory. They name the contract, its inputs and its applicability, then call a contract helper. Framework-specific support lives under tests/support/: application modules manage lifecycle and fixture inputs, and contract modules perform browser operations and assertions. scripts/ contains CLI support for framework builds.

The common acceptance.ts declares phases and cases. application.ts owns workspace, process, browser and artifact lifecycle. contracts.ts implements shared rendering, navigation and source-edit contracts; browser.ts implements computed-style and document assertions. scenario.ts registers specialized suites against disposable workspaces. The metaframework helpers restore captured sources and open the initial route before each development case. Production preparation sets its own theme, so it can run without the development edit cases. Warm checks prime a build when no earlier build ran. Restoring unchanged sources preserves their timestamps.

The shared harness owns workspace copies, processes, browser assertions and source-map checks. Common runs write report.json with actual dependency versions and completed contracts, HTML from SSR checks, browser traces, server logs and failure screenshots into each package's results/ directory. Acceptance suites use native Vitest `describe`, `test`, and lifecycle hooks. Common contracts and specialized scenarios each report named cases. Suite hooks share server and build preparation, and the harness creates their disposable workspaces before execution. CLI support modules use TypeScript and run directly in Node. Each package includes its tests, support modules and CLI scripts in its TypeScript check. Specialized runs retain their reports and process output in a unique results/ directory. The matrix writes exit status and package logs into a separate directory under e2e/results/ for each run. All application outputs and results are ignored by Git.

The root unplugin patch composes source maps in its Rspack transform loader. Its location and version are declared in pnpm-workspace.yaml.

## Support policy

Each fixture declares `support.tier` as `primary` or `compatibility`. `support.profiles` overrides the tier when a profile uses a different host or integration. Next's normal webpack and Turbopack profiles are primary; its standalone rules profile is compatibility. Vite and the normal metaframework applications are primary. Native webpack, Rspack and Rsbuild applications, the Rsbuild server-function profile, and Solid 2 RC are compatibility.

Both tiers remain in `vp run test:e2e`. A compatibility classification does not allow failed tests. Known limitations declare `expectedFailure: { reason, issue? }` and retain native `test.fails` assertions. Their unexpected passes fail the suite.

Contract priorities live in `harness/src/policy.ts`. CSS rendering, source updates, SSR, interactions, navigation, semantics and build invalidation have high priority. Source maps have low priority in both support tiers. Common reports record each case's contract, priority and expected failure. Specialized suites record the fixture tier and the shared priority definitions alongside their case reports; their combined cases can exercise several contracts.

Run `vp run test:e2e:primary` or `vp run test:e2e:compatibility` for one support tier. `vp run @stillwind-internal/e2e-harness#test:e2e --support primary` or `--support compatibility` also selects a tier. Package names and `STILLWIND_E2E_PROFILE` can further restrict the selection. Framework-independent CSS tests run with either tier unless package names exclude them. `--list` prints the selection without starting servers. Tier selections write separate `matrix-primary.json` and `matrix-compatibility.json` reports.

[Support inventory](support.md) lists every fixture profile and its declared limitations. Regenerate it with `vp exec node e2e/harness/src/matrix.ts --write-support-doc` after changing manifests, then run `vp fmt e2e/support.md`.
