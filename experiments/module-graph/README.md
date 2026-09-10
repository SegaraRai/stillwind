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
