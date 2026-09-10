# Module graph implementation

Goal: complete the concrete architecture and acceptance work in
`module-graph-architecture-research.md`. Future SFC frontends and optional global
optimization remain extension points, as specified there; they must not be
mistaken for implemented capabilities. Work is committed in coherent steps,
with a target of 5–30 commits including the initial integration.

| Work                                                                  | Status           | Evidence / next check                                                                             |
| --------------------------------------------------------------------- | ---------------- | ------------------------------------------------------------------------------------------------- |
| Module CSS transport and class identity                               | Done (`77ba639`) | Rust, Vite build, Chrome edit/remove/restore, fixtures; workspace commit gates                    |
| Self-contained `stillwind` package and CSS exports                    | Done (`30aff1a`) | Six package tests and seven Vite tests; recursive exported CSS imports stay within the package    |
| CSS-derived registries; remove Rust CSS embedding and magic imports   | Implemented      | Real package graph test changes definitions without rebuilding Rust; empty input has no defaults  |
| Import modifiers, utilities slot, source provenance and relative URLs | Pending          | Layer/supports/media placement, explicit diagnostics, URL and mapping fixtures                    |
| Demand-driven semantic reads and environment/session isolation        | Pending          | Unreached sources, imported/re-exported assets, cache, deletion, stale async updates              |
| Artifact selection, support closure and output contracts              | Implemented      | Explicit selections and coverage; declaration support; document/carrier output and revision tests |
| Next Turbopack and webpack adapters                                   | Pending          | Real compiler: RSC/client, route navigation, CSS edits, prerender, cold/warm cache                |
| Rspack and Rsbuild adapters                                           | Pending          | Real compiler: native CSS chunks, browser behavior, persistent cache and dependency updates       |
| Vite framework templates                                              | Pending          | React Router, SolidStart, TanStack Start: SSR, hydration, lazy CSS, route changes                 |
| Editor overlays and partial analysis                                  | Pending          | Unsaved buffers, invalid syntax, versions and stale requests                                      |
| Final acceptance and documentation                                    | Pending          | Runnable templates, source maps, repeatable commands and precise capability matrix                |

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
slots are preserved.

Anonymous output layers now receive deterministic internal names derived from
the entry identity, configured namespace and template occurrence. Document CSS
declares their order, and partial module carriers reopen the same layers even
when preceding authored groups are omitted. Nested anonymous layers and comments
in their headers are covered. Core/CSS library tests passed (54 and 75).

Entry migration validation: workspace Rust tests passed; after the token scanner
fix, Core tests (133), CSS tests (75), and Tailwind tests (87, one ignored) passed.
The scanner regression was observed as a missing `container` in the real
manifest graph and corrected before accepting fixture changes. Vite tests (7),
core package tests (6), CLI package exports (3), native binding tests (8), React
build, Chrome edit/remove/restore (4), and all six review fixtures passed.
Workspace Clippy, relevant TypeScript checks, and changed-file lint passed.

CSS graph resources now retain their owner, original span and decoded specifier.
The expanded source uses deterministic placeholders and piecewise mappings, so
length-changing URL substitutions preserve subsequent token positions. Vite
resolves these through its host resolver before CSS asset processing; a real
build emits two hashed SVG assets from authored and utility CSS in an imported
file. CLI output rebases resources to the CSS entry directory. Quoted/unquoted
`url()` and direct `image-set()` strings are recognized; comments, ordinary
strings and absolute/data/fragment URLs are left intact. Vite tests (8), CLI
tests (3), Core/CSS library tests and workspace Clippy passed. Resource URLs
authored in JS style arguments and generated source maps remain pending.

Vite now composes native UTF-16 replacement spans with removed macro imports
and synthetic runtime/CSS imports into a standard JavaScript source map.
The real lazy-chunk build maps its generated class back to the original style
call. CRLF, non-BMP text, unchanged statements and unmapped synthetic imports
are covered separately. Vite tests: nine passed, including TypeScript checks.
CSS rule provenance maps remain outstanding. Installing the map encoder also
refreshes the workspace lock to the current Vite override and renamed core package.

Vite sessions, TypeScript services, producer sets, CSS dependencies and resource
resolutions are now scoped by Vite environment. The per-environment hot-update
hook reloads only that environment and rejects outdated asynchronous loads.
A real Vite server test resolves the same import to different client/SSR CSS,
checks each module carrier, reloads only the client and releases an older load
after a newer successful reload. Ten Vite tests passed. The Chrome real-compiler
probe passed initial CSS, CSS-only edit, macro removal and restoration checks.

Native summaries expose checker probe spans. Literal utilities no longer create
probe candidates; Vite skips recursive semantic input reads and checker startup
when the entry has no probes. Semantic inputs can now be supplied to a native
session while selecting only one producer for compilation and registration.
The cross-module fixture verifies animation support on the selected consumer
without registering its source producer. Vite tests: ten passed; Oxc: eleven;
native binding: eight. Probe-bearing graphs still use conservative recursive
local dependency discovery; finer checker-driven discovery remains pending.

Core and Node sessions now plan explicit producer selections with observed or
host-guaranteed complete coverage. Complete coverage names a target and graph
revision; missing producers and stale retained watch artifacts are rejected.
Document placement includes authored global CSS; carrier placement supports
module/chunk selections without those globals. Plans include design-system,
source, semantic and compiler identities. Completeness remains a host assertion,
not something inferred from callbacks.

The standalone-consumer regression exposed missing imported animation keyframes.
Selected native compilation now attaches referenced animation/view-transition
declarations from semantic inputs, without registering those inputs as producers
or including unrelated ordinary styles or unused animations. Support source
changes invalidate the consumer artifact and survive CSS snapshot recompilation.
The Node fixture verifies the closure, exclusion and a producer-only content edit.
The fingerprint regression fixture now preserves entry identity while varying
file IDs: entry identity intentionally namespaces anonymous layers.
Vite uses these plans and retains their coverage/revision metadata in transform
results. All Core and native tests (including integration/property tests) passed,
as did the ten Vite tests and Core/native Clippy checks.

JS-authored relative resources now carry their source-module owner through
artifacts, standalone support declarations, native output and stylesheet plans.
Vite resolves them before its asset pipeline; CLI rebases them to the CSS entry
directory. Relative-resource classes retain source identity even in content mode,
so identical spellings in different directories cannot collide. A real Vite
build emits two distinct SVGs and classes for two `./icon.svg` references; an
imported animation retains its producer owner. Eleven Vite tests, 57 Core library
tests, four CLI tests and Core/native/CLI Clippy passed. CSS source maps remain
outstanding.
