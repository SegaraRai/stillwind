# Coverage and limitations

The fixture matrix selects framework versions and observable contracts. A passing matrix does not certify every peer dependency version or rendering mode.

## Public integrations

The framework subpaths in packages/core/package.json are React, Preact, Vue, Solid, Svelte and Astro. All six have browser fixtures. Vue SFC and JSX use separate applications. Solid 1 and Solid 2 RC use separate applications.

The build integrations in packages/integration/package.json are Vite, webpack, Rspack, Rsbuild, Next, Astro and standalone Turbopack rules. All have browser fixtures. Standalone Turbopack rules run through the Next application. Nuxt, SvelteKit, React Router, SolidStart and TanStack Start exercise framework integrations with those hosts.

## Rendering modes

| Mode                        | Current evidence                                                                                                                  | Not covered                                                                                    |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| Vite client applications    | React, Preact, Vue SFC/JSX, Solid 1/2 RC and Svelte                                                                               | Direct SSR applications for Preact, Vue, Solid and Svelte; Vite RSC through @vitejs/plugin-rsc |
| React SSR through Vite      | Detailed composition and conditional scenarios inside vite8-react19                                                               | An ordinary Vite SSR application with the whole common contract                                |
| Nuxt                        | Vue SSR and hydration                                                                                                             | SPA configuration                                                                              |
| SvelteKit                   | Svelte SSR and hydration with the Node adapter                                                                                    | Other adapters and SPA configuration                                                           |
| Astro                       | Static prerendering and all five supported island frameworks                                                                      | Dynamic SSR with an adapter and server islands                                                 |
| Next                        | App Router, RSC with client hydration, Flight prefetch, navigation, webpack/Turbopack, cold/warm builds and server-only isolation | Server Actions, streaming or Suspense timing, Pages Router                                     |
| React Router and SolidStart | SSR, hydration and lazy routes                                                                                                    | Other rendering configurations                                                                 |
| TanStack Start              | React/Solid SSR and hydration; React server functions through Vite/Rsbuild                                                        | RSC integration; server functions are a separate contract                                      |
| Native bundlers             | React, Preact, Vue SFC, Solid 1 and Svelte through webpack/Rspack/Rsbuild                                                         | Vue JSX and Solid 2 RC through these hosts                                                     |

Vite 7 and Astro 6 remain accepted peer dependency versions. The browser matrix selects Vite 8 and Astro 7. Compatibility with the older majors requires separate verification.

## Fixture requirements and known limitations

Rsbuild excludes Vue's synthetic script queries from its JavaScript transform rule. Its Vue fixture protects this contract.

Nuxt creates independent Stillwind plugins for its client and SSR Vite servers. The fixture forwards a development stylesheet request to the SSR server and removes that stylesheet after hydration. This verifies the explicit integration configuration; it does not demonstrate automatic Nuxt stylesheet insertion.

Astro embeds explicit module CSS values from pages and islands in its prerendered HTML. The island fixtures select hashed chunk names because CSS fragment identifiers can exceed Windows filename limits. The tests do not demonstrate automatic SSR stylesheet insertion or default chunk naming for these exports.

Some frameworks reload the document when a module's exports change. Every edit declares the expected document and component state behavior. Passing a reload expectation does not establish that the same edit supports HMR without a full reload.

Source-map checks distinguish unsupported upstream mappings from successful mapping assertions. Inspect the saved evidence before making a source-map compatibility claim.

Turbopack can map a minified JavaScript class string to an argument inside its original `style()` call. Its JS check requires the position to remain inside that call. CSS checks and other JS hosts require the call's exact start position. The saved map evidence records which position check applied.

Turbopack's CSS graph treats `server-only` and `client-only` as external boundary markers. Next still validates those markers in its server and client builds. The Next fixture requires its server implementation sentinel to appear in server JavaScript and to remain absent from client JavaScript, CSS, source maps and Flight responses.

In Solid 2.0.0-rc.13, spreading a native Boolean `aria-busy` value renders an empty attribute for `true`. A control using a plain JSX spread reproduces this behavior without Stillwind. The condition probes supply explicit `"true"` and `"false"` strings to preserve the ARIA values across frameworks.
