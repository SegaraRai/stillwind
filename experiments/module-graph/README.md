# Module graph transport experiments

Standalone, pnpm-backed Vite+ workspace. This is a transport/lifecycle probe, not the Stillwind compiler or a production integration. `styleToken('CSS declarations')` is a deliberately tiny regex-based surrogate. Do not use it to assess parsing, type inference, or macro semantics.

From this directory:

```powershell
vp i --frozen-lockfile
vp run verify
```

Requires an installed Google Chrome (Playwright uses `channel: 'chrome'`) and free ports 43171–43173. Node, Next, Vite+, Rspack, React and Playwright versions are recorded in `results/latest.json`; the package lock pins dependencies. No browser download is required on the measured machine.

The runner creates fresh fixtures under `output/<timestamp>/`, launches headless Chrome and local servers, and shuts them down. These fixtures and build caches are ignored. Raw Next logs are in ignored `results/*.log`. The compact, checked result is `results/latest.json`.

Experiments:

- Reverse isolated CSS selectors, then deliberately compose them on one element; measure computed colors.
- Vite: return the current aggregate at CSS load time while a module transform is delayed by 350 ms. A missing slow rule is the expected negative-control outcome, not a working integration.
- Vite: module-owned CSS query imports retain slow and dynamic-import rules in build output. Development only transforms demanded modules; edits and macro removal are observed in Chrome.
- Rspack: module-owned CSS, native CSS chunks, separate-process persistent-cache builds, and invalidation after a registered dependency changes. Checks inspect the current chunk file list, not obsolete files left in the output directory. Minification is disabled to inspect declarations directly.
- Next/Turbopack: actual App Router RSC and Client Component rendering, lazy route requests, dependency HMR, production prerendering, and separate-process cached rebuilds. Loader invocations are counted; the cached build must execute zero JS loaders while CSS still renders.

The CSS query carries its complete content. The CSS loader never reads a process-global registry. The producer loader uses a stable project-relative owner salt and emits a relative CSS import. An initial Windows drive-absolute import attempt failed in Turbopack with `windows imports are not implemented yet`; relative imports fixed the experiment.

Known limits: fixed-port single-run harness; no source-map validation, CSS URL rebasing, Shadow DOM, arbitrary type dependencies, real LSP, full Stillwind compilation, Rspack browser HMR, or all framework templates. Vite edit/removal may reload the page; the test does not assert state-preserving HMR. Next lazy route uses direct navigation, not client-router prefetch/streaming. The cold production build means first production build in a fresh fixture after a dev run; the warm build reuses its production cache.

The real Stillwind adapter has a separate browser acceptance probe. First run
`vp run test` from `packages/vite` to build the native addon, then run
`vp run verify:stillwind` here. It checks real macro compilation, CSS dependency
edits with stable class names, macro removal and restoration. Its result is
`results/stillwind.json`. Page reloads are allowed; it does not assert preservation
of application state. The original 18 transport checks remain surrogate experiments.

## Real framework adapters

The following separate tasks use the real Stillwind compiler, not `styleToken`:

- `vp run verify:adapters`: Next 16.3.4 with Turbopack.
- `vp run verify:webpack`: Next 16.3.4 with webpack.
- `vp run verify:rspack`: Rspack native CSS and persistent cache.
- `vp run verify:rsbuild`: Rsbuild wrapper over the same adapter.
- `vp run verify:react-router`: React Router 8.3.1 Framework Mode and Vite+.
- `vp run verify:solidstart`: SolidStart 2.0.4, Solid 1.9.15, Nitro, and Vite+.
- `vp run verify:tanstack-vite`: TanStack Start 1.168.50 with Vite+ and Nitro.
- `vp run verify:tanstack-rsbuild`: TanStack Start 1.168.50 with Rsbuild 2.2.4.

Next checks RSC/client rendering, hydration, dependency edits, macro removal,
client navigation, prefetch, and cold/warm production builds. Rspack/Rsbuild
check browser CSS and URL assets, cache restoration, dependency invalidation,
unreachable producers, and macro removal. Their results are
`results/adapters-{turbopack,webpack}.json`, `results/rspack-real.json`, and
`results/rsbuild-real.json`.

React Router verifies JavaScript-disabled SSR (including `.server.js` styles
connected by `?stillwind.css`), hydration, client navigation, lazy CSS, CSS edits,
three macro removal/restoration cycles, and projected server-source edits.
Each cold/warm build also checks JavaScript source-map positions and excludes
server code from client JavaScript and source maps. Results are stored in
`results/react-router-real.json`. The runner creates a complete, inspectable
fixture under `output/react-router-<timestamp>` using the dependencies installed
here. Next uses ports 43174–43175; React Router uses 43176. Do not run two copies
of the same task concurrently.

SolidStart uses port 43177 and writes `results/solidstart-real.json`. Its generated
fixture verifies JavaScript-disabled SSR, hydration, both branches of a reactive
comparison, lazy navigation without document replacement, CSS edits, macro
removal/restoration, and cold/warm production builds with `stillwind/solid`.

TanStack Start uses ports 43178 (Vite) and 43179 (Rsbuild). Results are in
`results/tanstack-{vite,rsbuild}-real.json`. Both verify JavaScript-disabled SSR,
hydration, actual server-function requests, lazy navigation, stylesheet edits,
macro removal/restoration, projected server-style edits, and cold/warm builds.
The client JavaScript must exclude the server function's sentinel body.
The Rsbuild adapter runs between framework pre-loaders and SWC; Vite places
Stillwind after `tanstackStart()` and before React's plugin.

The frozen lockfile applies `patches/@tanstack__start-plugin-core@1.171.40.patch`
to normalize Windows route module paths in the upstream Rsbuild CSS manifest.
Without that one-line patch, SSR omits CSS belonging to split route components.
This is an explicit dependency patch, not a claim that the unpatched upstream
release passes this Windows acceptance fixture. `vp i --frozen-lockfile`
reapplies it on a fresh checkout.
