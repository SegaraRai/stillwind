# Compiler and source-map benchmarks

Run these tasks from the repository root:

- `vp run bench:compiler`: native loader, standalone compilation, unchanged updates, single-file edits and stylesheet planning across synthetic module graphs.
- `vp run bench:host`: reused and fresh checker services, direct scripts and Vue/Svelte/Astro input.
- `vp run bench:source-map`: production CSS map generation versus the candidate in `source-map/candidate.ts`.

Results are written to ignored `results/`. Timings are descriptive; ordinary verification does not impose machine-dependent performance thresholds. The compiler benchmark records its Git revision and output fingerprints. The host benchmark uses synthetic modules without a disk dependency graph.

The CSS-map candidate indexes mapping starts, advances source cursors and scans placeholders with a compiled pattern. It is used only by the benchmark and its colocated behavior tests. The production host still indexes source positions and searches resource replacements in `packages/host/src/css-source-map.ts`. Both benchmark variants use the current source identity and upstream-map contracts. Candidate correctness tests run in the ordinary JS test suite.
