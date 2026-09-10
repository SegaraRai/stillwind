# Module graph implementation

Goal: complete the concrete architecture and acceptance work in
`module-graph-architecture-research.md`. Future SFC frontends and optional global
optimization remain extension points, as specified there; they must not be
mistaken for implemented capabilities. Work is committed in coherent steps,
with a target of 5–30 commits including the initial integration.

| Work                                                                  | Status           | Evidence / next check                                                                                           |
| --------------------------------------------------------------------- | ---------------- | --------------------------------------------------------------------------------------------------------------- |
| Module CSS transport and class identity                               | Done (`77ba639`) | Rust, Vite build, Chrome edit/remove/restore, fixtures; workspace commit gates                                  |
| Self-contained `stillwind` package and CSS exports                    | In progress      | Recursive exported CSS graph must stay within the package                                                       |
| CSS-derived registries; remove Rust CSS embedding and magic imports   | Pending          | Changed CSS definitions must change behavior without rebuilding Rust; no-import input must not acquire defaults |
| Import modifiers, utilities slot, source provenance and relative URLs | Pending          | Layer/supports/media placement, explicit diagnostics, URL and mapping fixtures                                  |
| Demand-driven semantic reads and environment/session isolation        | Pending          | Unreached sources, imported/re-exported assets, cache, deletion, stale async updates                            |
| Artifact selection, support closure and output contracts              | Pending          | Observed vs complete coverage, producer identities, document/chunk/module outputs                               |
| Next Turbopack and webpack adapters                                   | Pending          | Real compiler: RSC/client, route navigation, CSS edits, prerender, cold/warm cache                              |
| Rspack and Rsbuild adapters                                           | Pending          | Real compiler: native CSS chunks, browser behavior, persistent cache and dependency updates                     |
| Vite framework templates                                              | Pending          | React Router, SolidStart, TanStack Start: SSR, hydration, lazy CSS, route changes                               |
| Editor overlays and partial analysis                                  | Pending          | Unsaved buffers, invalid syntax, versions and stale requests                                                    |
| Final acceptance and documentation                                    | Pending          | Runnable templates, source maps, repeatable commands and precise capability matrix                              |

Every step records tests actually run. A successful surrogate transport probe
does not substitute for acceptance using the Stillwind compiler.
