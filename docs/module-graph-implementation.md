# Module graph implementation and acceptance

Verified on Windows x64 on 2026-09-10. This is the current implementation status
for [the architecture research](module-graph-architecture-research.md). The
research document retains the original experiments and design rationale.

## Implemented boundaries

| Area                            | Implementation and evidence                                                                                                                                                                                                                                                              |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CSS entry and package ownership | `stillwind` owns its exported CSS, React/Solid macros and runtime. Imports resolve through package exports. Rust does not embed the default stylesheet or recognize a magic `tailwindcss` alias.                                                                                         |
| CSS graph                       | Immutable per-snapshot registries; source-ordered imports, layer/supports/media wrappers, explicit `@stillwind-slot utilities;`, stable anonymous layers, physical source provenance and owner-relative resources. Unsupported conditional compile-time definitions produce diagnostics. |
| Module output                   | Self-contained physical CSS query carriers contain selected rules and their support. A CSS load never depends on collecting all transform callbacks. Document CSS and module contributions have separate placement.                                                                      |
| Identity                        | Each style call has an independent identity; content deduplication is disabled for composition. Namespace settings separate independently built applications. Resource-bearing rules preserve their source owner.                                                                        |
| Semantic inputs                 | Native probe summaries select relevant bindings and barrel exports. The host supplies local resolutions to the checker. Unrelated runtime imports do not become semantic producers. External package declarations retain TypeScript resolution.                                          |
| Artifacts and coverage          | Explicit producer selection, support closure and revision identities. Observed coverage is distinct from host-guaranteed complete coverage; stale or missing producer selections are rejected.                                                                                           |
| Environment and editor state    | Separate Vite environments; immutable CSS revisions; stale asynchronous loads rejected. `EditorProject` handles unsaved documents, dependencies, partial diagnostics, versions, cancellation and previous-success previews.                                                              |
| Frameworks                      | Real-compiler adapters for Vite, Next/Turbopack, Next/webpack, Rspack and Rsbuild. Runnable acceptance fixtures cover React Router, SolidStart and both TanStack Start bundlers.                                                                                                         |
| Source maps                     | JavaScript replacements and CSS provenance compose with preceding transforms. Authored CSS, theme tokens, macro/symbol origins, non-ASCII positions and URL-length edits are tested. Final CSS-map support depends on the downstream bundler, as listed below.                           |

The host package owns compiler orchestration, checker snapshots, source-map
composition and finalization. Resolution, reads, dependency registration and
diagnostics remain host callbacks. Bundler loaders use isolated native sessions;
restored carriers can be decoded in another process without a shared registry.

The checker distinguishes adding an overlay to an existing disk file from
creating a new file. Removing an overlay restores the disk version instead of
notifying TypeScript that the file was deleted. Regression tests exercise
addition, removal and restoration without restarting the checker process.

Custom utility and variant names enter generated vocabularies without acquiring
implicit builtins. Type declarations expose utility names and variant prefixes
without materializing their full Cartesian product; the compiler validates
complete utility strings.

## Framework acceptance

All listed probes use the actual compiler and Chrome. The original 18 transport
experiments remain separate surrogate evidence. Results and generated fixture
instructions are in [the experiment workspace](../experiments/module-graph/README.md).

| Target                            | Verified behavior                                                                                                                                                              | Final JavaScript map                        | Final CSS map                                                                                                                            |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Vite                              | Late/lazy producers, resource emission, independent client/SSR resolution, CSS dependency edits, macro removal/restoration, aliases                                            | Original macro, including lazy build output | Development map, including preprocessing; production CSS maps unavailable in the tested Vite release                                     |
| Next 16.3.4 / Turbopack           | RSC/client hydration, state-preserving CSS edit, macro removal/restoration, client navigation, observed prefetch, cold/warm production                                         | Original macro in both builds               | **Unsupported in this fixture:** final map retains the macro line/column but names `packages/core/index.css` instead of `app/client.jsx` |
| Next 16.3.4 / webpack             | Same six browser acceptance checks                                                                                                                                             | Original macro in both builds               | Original macro in both builds                                                                                                            |
| Rspack 2.2.3                      | Browser CSS and SVG, persistent-cache restoration, dependency edit, unreachable producer, macro removal                                                                        | Shared loader composition tested            | Original macro selector in cold/warm builds                                                                                              |
| Rsbuild 2.2.4                     | Same browser/cache checks; Lightning CSS composition                                                                                                                           | Shared loader composition tested            | Original macro selector in cold/warm builds                                                                                              |
| React Router 8.3.1                | JS-disabled SSR, hydration, lazy client navigation, CSS edits, three removal/restoration cycles, server projection edits, cold/warm builds, no server code in client artifacts | Original macro in both builds               | Vite production limitation                                                                                                               |
| SolidStart 2.0.4                  | `stillwind/solid` class props, JS-disabled SSR, reactive comparisons, hydration, lazy navigation, CSS edits, removal/restoration, cold/warm builds                             | Original macro in both builds               | Vite production limitation                                                                                                               |
| TanStack Start 1.168.50 / Vite    | JS-disabled SSR, hydration, real server-function request, lazy navigation, CSS edits, macro removal/restoration, server projection edits, cold/warm builds                     | Original macro in both builds               | Vite production limitation                                                                                                               |
| TanStack Start 1.168.50 / Rsbuild | Same acceptance checks with the dependency patches below                                                                                                                       | Original macro in both builds               | Original macro in both builds, composed through route preprocessing                                                                      |

The experiment lockfile applies two explicit upstream patches for Windows
TanStack/Rsbuild acceptance: `@tanstack/start-plugin-core@1.171.40` normalizes
route paths in the CSS manifest, and `unplugin@3.3.0` composes rather than replaces
transform maps. These results do not claim unpatched upstream support.

Framework splitting runs before Stillwind determines producer ownership. A
server-only module can be connected to a route with an explicit
`import './server-style.ts?stillwind.css'`; only CSS is projected into the client.
Projection maps retain file/line/column but omit server source text. React Router
checks both client JavaScript and source maps for its server-only sentinel.
Layer order is repeated in carriers so early route CSS is not overridden by
later Preflight. Development carrier versions prevent an older HMR prune from
removing a newly restored macro's stylesheet; they do not change class identity.

CSS rules use macro/symbol anchors when no exact source range exists. Generated
wrappers and helper rules can be unmapped. Lightning CSS preserves selector
origins rather than every declaration column. The Turbopack runner records its
specific source-file mismatch as `cssSourceMap.supported: false`, not as a
source-map pass; other mismatches still fail. Disabling Turbopack minification
was also tried and did not repair that mismatch. SolidStart edit checks allow
300 ms for the Windows file watcher to settle between removal and restoration;
they do not claim that every intermediate rapid disk write is observed.

## Repeatable verification

Prerequisites: the repository's Rust toolchain and platform linker, Node.js,
Vite+ and pnpm. Framework browser probes also require installed Chrome. The
standalone experiment has its own frozen lockfile and fixed ports; do not run
two copies of the same probe concurrently.

From the repository root:

```sh
vp i --frozen-lockfile
vp run -r build
vp run -r typecheck
vp run -r test
vp run -r check
vp run fixtures:check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

`test` covers host/Vite suites; `check` additionally runs native/core package
tests and application TypeScript checks. Build includes the React example,
playground, fixture viewer and its six unchanged review fixtures. Each example's
build/dev task first builds the host package and native dependency.

From `experiments/module-graph` after building the repository:

```sh
vp i --frozen-lockfile
vp run verify:stillwind
vp run verify:adapters
vp run verify:webpack
vp run verify:rspack
vp run verify:rsbuild
vp run verify:react-router
vp run verify:solidstart
vp run verify:tanstack-vite
vp run verify:tanstack-rsbuild
```

Workspace Rust tests, workspace Clippy, all package/application checks, twelve
host tests, thirteen Vite tests, five native Node tests, six core Node tests and
six review fixtures passed. One pre-existing ignored Rust registry audit remains
ignored in the default workspace test command. Windows incremental-cache cleanup
notes and playground bundle-size warnings are non-failing build output.

A separate checkout with no native binary, Rust target directory or host/app
build output passed frozen installation and a cache-disabled workspace build.
After the final projection change, the same checkout passed cache-disabled
`build`, `typecheck`, `test` and `check` tasks at `a66e9fe`. No tracked files were
changed by installation or validation.

## Explicit limits

- Svelte/Vue/Astro frontends, a complete LSP transport/UI and optional global
  optimization remain extension points.
- Loader callbacks do not establish complete graph coverage, expression-level
  tree shaking or arbitrary framework streaming/edge behavior.
- Literal macros dominate framework browser fixtures. Cross-module symbolic
  semantics, aliases and editor overlays have separate native/host/Vite tests.
- The tested Vite production pipeline and Next/Turbopack CSS source mapping
  have the limitations above. JavaScript source-map acceptance is separate.
- Self-contained CSS/map payloads can be large; webpack reports cache string
  serialization warnings. No payload compression or large-project performance
  claim is included in this acceptance.
