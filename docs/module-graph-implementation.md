# Module graph implementation

Goal: complete the concrete architecture and acceptance work in
`module-graph-architecture-research.md`. Future SFC frontends and optional global
optimization remain extension points, as specified there; they must not be
mistaken for implemented capabilities. Work is committed in coherent steps,
with a target of 5–30 commits including the initial integration.

| Work                                                                  | Status           | Evidence / next check                                                                                           |
| --------------------------------------------------------------------- | ---------------- | --------------------------------------------------------------------------------------------------------------- |
| Module CSS transport and class identity                               | Done (`77ba639`) | Rust, Vite build, Chrome edit/remove/restore, fixtures; workspace commit gates                                  |
| Self-contained `stillwind` package and CSS exports                    | Done (`30aff1a`) | Six package tests and seven Vite tests; recursive exported CSS imports stay within the package                  |
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

Registry ownership now belongs to each immutable theme snapshot. Utility,
variant, property, formatting and ordering consumers read that snapshot, with
no process-global semantic registry. Core/CSS tests: 204 passed; Tailwind tests:
87 passed, one pre-existing ignored parity audit. An isolation test resolves
different declarations from the same utility name in separate registries and
checks that an empty registry does not acquire builtins. The legacy CSS input
fallback remains temporarily in the CSS parser until the entry/template step.

The source-ordered output template now preserves layers and conditions around
utility slots and theme support. It has a parser entry that injects no defaults.
Core tests: 132 passed; CSS tests: 77 passed after correcting a test to allow
preserved comments mentioning compiler directives. The graph loader still uses
the old parser in this intermediate commit; the next step switches all hosts
and removes that parser's implicit default input.
