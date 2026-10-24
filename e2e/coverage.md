# Coverage and limitations

The fixture matrix selects framework versions and observable contracts. A passing matrix does not certify every peer dependency version or rendering mode.

## Public integrations

The framework subpaths in packages/core/package.json are Alpine, Astro, Lit, Preact, Qwik, React, Solid, Svelte, Vanilla and Vue. All have browser fixtures. Alpine uses its `x-sw` directive. Lit uses an element directive in Shadow DOM and does not support SSR. Vue SFC and JSX use separate applications. Solid 1 and Solid 2 RC use separate applications.

The build integrations in packages/integration/package.json are Vite, webpack, Rspack, Rsbuild, Next, Astro, Fresh, Nuxt, the SvelteKit SSR handle and standalone Turbopack rules. All have browser fixtures. Standalone Turbopack rules run through the Next application. Fresh 2, Gatsby, Nuxt, SvelteKit, React Router, SolidStart and TanStack Start exercise framework integrations with those hosts.

## Rendering modes

| Mode                        | Current evidence                                                                                                                  | Not covered                                                                                             |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Vite client applications    | Alpine, Lit, Preact, React, Solid 1/2 RC, Svelte and Vue SFC/JSX                                                                  | Lit SSR; direct SSR applications for Preact, Vue, Solid and Svelte; Vite RSC through @vitejs/plugin-rsc |
| Qwik                        | Vite 7, SSR, resumption, navigation and source edits                                                                              | Vite 8 with Qwik 1                                                                                      |
| Fresh 2                     | Vite 7, Preact islands, SSR, hydration and source edits                                                                           | Fresh's legacy build system                                                                             |
| Gatsby                      | React SSR in development, prerendering, hydration, navigation and source edits                                                    | Other Gatsby plugins and rendering configurations                                                       |
| React SSR through Vite      | Detailed composition and conditional scenarios inside vite8-react19                                                               | An ordinary Vite SSR application with the whole common contract                                         |
| Nuxt                        | Vue SSR and hydration                                                                                                             | SPA configuration                                                                                       |
| SvelteKit                   | SvelteKit 2 and 3 SSR and hydration with the Node adapter                                                                         | Other adapters and SPA configuration                                                                    |
| Astro                       | Static prerendering and all five supported island frameworks                                                                      | Dynamic SSR with an adapter and server islands                                                          |
| Next                        | App Router, RSC with client hydration, Flight prefetch, navigation, webpack/Turbopack, cold/warm builds and server-only isolation | Server Actions, streaming or Suspense timing, Pages Router                                              |
| React Router and SolidStart | SSR, hydration and lazy routes                                                                                                    | Other rendering configurations                                                                          |
| TanStack Start              | React/Solid SSR and hydration; React server functions through Vite/Rsbuild                                                        | RSC integration; server functions are a separate contract                                               |
| Native bundlers             | React, Preact, Vue SFC, Solid 1 and Svelte through webpack/Rspack/Rsbuild                                                         | Vue JSX and Solid 2 RC through these hosts                                                              |

Vite 7 and Astro 6 remain accepted peer dependency versions. Most browser fixtures select Vite 8 and Astro 7. Qwik 1 and Fresh 2 use Vite 7. Fresh 2 requires Deno 2.9 or newer; its acceptance fixture uses Deno 2.9.8. Compatibility with Astro 6 requires separate verification.

## Fixture requirements and known limitations

Rsbuild excludes Vue's synthetic script queries from its JavaScript transform rule. Its Vue fixture protects this contract.

Nuxt's module registers independent Stillwind plugins for its Vite servers. It adds the development SSR stylesheet and disposes it after hydration. SvelteKit uses the Vite plugin and the public Stillwind server handle. Its application composes that handle with other application handles when required.

The Astro adapter supplies styles through Astro middleware. Its development browser module refreshes those SSR styles on CSS updates, including pages without islands. Theme edits preserve the document and island state. Pages and islands use automatic producers, and they do not export CSS values for SSR delivery. Island component edits must match Astro's ordinary HMR behavior: React, Vue and Preact preserve state; the selected Svelte and Solid versions reset the edited island. These expectations come from equivalent component edits without Stillwind. The other islands retain their state.

Some frameworks reload the document when a module's exports change. Every edit declares the expected document and component state behavior. Passing a reload expectation does not establish that the same edit supports HMR without a full reload.

The Vue fixtures verify JavaScript and CSS maps in minified and unminified builds. Vite composes Vue maps at both the load and transform boundaries. Rsbuild production builds apply the SFC map bridge to Vue synthetic script resources as well as the original SFC. Development template maps also pass. Development script-query maps retain expected-failure cases. Composing the complete SFC map into these script maps makes Vue remount components on template edits. Changes to script module hashes are a possible cause; the exact Rspack update path has not been established. The development script bridge leaves the framework maps unchanged to preserve ordinary HMR. Correct script maps need a boundary that preserves that behavior. webpack's default CSS minification loses selector mappings; its unminified CSS check passes. Every expected-failure case retains the assertion for the original macro position. Vitest treats an unexpected pass as a failure, so a correction requires removing its expected-failure designation.

The common runner checks fresh SSR requests after source, shared-module and theme edits without enabling JavaScript. It compares every rendered Stillwind class between SSR and the hydrated page. It also rebuilds after every declared edit. The webpack and Rspack Vue fixtures enable persistent caches and inspect build stats to require a cached SFC producer in the warm build. Repeated builds in other fixtures do not establish a cache hit.

Turbopack can map a minified JavaScript class string to an argument inside its original `style()` call. Its JS check requires the position to remain inside that call. CSS checks and other JS hosts require the call's exact start position. The saved map evidence records which position check applied.

Turbopack's CSS graph treats `server-only` and `client-only` as external boundary markers. Next still validates those markers in its server and client builds. The Next fixture requires its server implementation sentinel to appear in server JavaScript and to remain absent from client JavaScript, CSS, source maps and Flight responses.

In Solid 2.0.0-rc.13, spreading a native Boolean `aria-busy` value renders an empty attribute for `true`. A control using a plain JSX spread reproduces this behavior without Stillwind. The condition probes supply explicit `"true"` and `"false"` strings to preserve the ARIA values across frameworks.

## Ownership of supported behavior

| Owner                 | Contract                                                                                                                                                                                                                                        |
| --------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Stillwind             | Generated classes have matching CSS after the framework renders an edited component. HMR, full reload and state changes follow the framework's ordinary behavior.                                                                               |
| Stillwind             | Fresh SSR requests receive current CSS, including theme and composition dependencies. SSR and client compilation agree when their source and naming options agree.                                                                              |
| Stillwind             | Framework adapters supply automatic SSR CSS; build invalidation and cache restoration preserve emitted styles. Source maps retain original positions where the host supplies the required mappings.                                             |
| Application / fixture | Register official framework plugins, adapters, CSS inputs, hydration and SSR entry points. Use the public Stillwind SSR handle where a framework requires a server hook.                                                                        |
| Application / fixture | Refetch route data through the framework when server functions change. TanStack Start's Rsbuild profile uses router invalidation or a fresh RPC, rather than reloading the document.                                                            |
| Application / fixture | Own explicit CSS values, custom SSR documents, Shadow DOM transport and frozen application data.                                                                                                                                                |
| Outside the guarantee | Automatically update an existing document with JavaScript disabled; preserve state beyond the framework's own HMR behavior; refetch server loaders independently of the application; keep class names unchanged after source or naming changes. |

The remaining production source-map failure is a limitation of webpack's default CSS minifier. Its coarse output mappings omit selectors. The input map alone cannot describe those output positions. The adapter currently preserves those host maps without inventing source coordinates.
