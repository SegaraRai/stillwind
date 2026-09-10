# Module graph implementation

Goal: complete the concrete architecture and acceptance work in
`module-graph-architecture-research.md`. Future SFC frontends and optional global
optimization remain extension points, as specified there; they must not be
mistaken for implemented capabilities. Work is committed in coherent steps,
with a target of 5–30 commits including the initial integration.

| Work                                                                  | Status           | Evidence / next check                                                                            |
| --------------------------------------------------------------------- | ---------------- | ------------------------------------------------------------------------------------------------ |
| Module CSS transport and class identity                               | Done (`77ba639`) | Rust, Vite build, Chrome edit/remove/restore, fixtures; workspace commit gates                   |
| Self-contained `stillwind` package and CSS exports                    | Done (`30aff1a`) | Six package tests and seven Vite tests; recursive exported CSS imports stay within the package   |
| CSS-derived registries; remove Rust CSS embedding and magic imports   | Implemented      | Real package graph test changes definitions without rebuilding Rust; empty input has no defaults |
| Import modifiers, utilities slot, source provenance and relative URLs | Pending          | Layer/supports/media placement, explicit diagnostics, URL and mapping fixtures                   |
| Demand-driven semantic reads and environment/session isolation        | Pending          | Unreached sources, imported/re-exported assets, cache, deletion, stale async updates             |
| Artifact selection, support closure and output contracts              | Pending          | Observed vs complete coverage, producer identities, document/chunk/module outputs                |
| Next Turbopack and webpack adapters                                   | Pending          | Real compiler: RSC/client, route navigation, CSS edits, prerender, cold/warm cache               |
| Rspack and Rsbuild adapters                                           | Pending          | Real compiler: native CSS chunks, browser behavior, persistent cache and dependency updates      |
| Vite framework templates                                              | Pending          | React Router, SolidStart, TanStack Start: SSR, hydration, lazy CSS, route changes                |
| Editor overlays and partial analysis                                  | Pending          | Unsaved buffers, invalid syntax, versions and stale requests                                     |
| Final acceptance and documentation                                    | Pending          | Runnable templates, source maps, repeatable commands and precise capability matrix               |

Every step records tests actually run. A successful surrogate transport probe
does not substitute for acceptance using the Stillwind compiler.

Registry ownership now belongs to each immutable theme snapshot. Utility,
variant, property, formatting and ordering consumers read that snapshot, with
no process-global semantic registry. Core/CSS tests: 204 passed; Tailwind tests:
87 passed, one pre-existing ignored parity audit. An isolation test resolves
different declarations from the same utility name in separate registries and
checks that an empty registry does not acquire builtins. The legacy CSS input
fallback was removed in the entry migration below.

The source-ordered output template now preserves layers and conditions around
utility slots and theme support. It has a parser entry that injects no defaults.
Core tests at the template step: 132 passed; CSS tests: 77 passed after correcting
a test to allow preserved comments mentioning compiler directives.

The graph loader now uses `parse_theme` with the resolved CSS source graph.
`parse_tailwind_theme`, default CSS embedding, framework import selectors and
the legacy renderer have been removed. Only explicit test fixtures load default
CSS for compatibility tests. Standard package exports and authored utility slots
are real CSS inputs. Vite, Playground, CLI and review/parity tooling have been
migrated. CLI package exports use Oxc Resolver with CSS style conditions.
Conditional compile-time definitions are diagnosed, while conditional output
slots are preserved. Source URL rebasing and anonymous layer identity across
separate module carriers remain in the next step.

Entry migration validation: workspace Rust tests passed; after the token scanner
fix, Core tests (133), CSS tests (75), and Tailwind tests (87, one ignored) passed.
The scanner regression was observed as a missing `container` in the real
manifest graph and corrected before accepting fixture changes. Vite tests (7),
core package tests (6), CLI package exports (3), native binding tests (8), React
build, Chrome edit/remove/restore (4), and all six review fixtures passed.
Workspace Clippy, relevant TypeScript checks, and changed-file lint passed.
