# Compiler and source-map benchmarks

Run these tasks from the repository root:

- `vp run bench:compiler`: native loader, standalone compilation, unchanged updates, single-file edits and stylesheet planning across synthetic module graphs.
- `vp run bench:host`: reused and fresh checker services, direct scripts and Vue/Svelte/Astro input.
- `vp run bench:source-map`: production CSS map generation versus the candidate in `source-map/candidate.ts`.

The tasks run Vitest benchmarks from `.bench.ts` files with `vite-plus/test`. Benchmark mode uses native Node ESM, so Vite does not transform the measured modules. Run one benchmark task at a time to avoid competition for CPU time. Ordinary tests run only `source-map/*.test.ts`.

Vitest JSON reports are written to ignored `results/compiler.json`, `results/host.json`, and `results/source-map.json`. Each assertion result includes benchmark statistics, raw latency samples, and metadata with the Git revision, Node version, platform, timestamp, and measurement options. Fresh compiler graph cases also record output fingerprints. Latency values use milliseconds; `latency.p50` is the median. Timings are descriptive; ordinary verification does not impose machine-dependent performance thresholds.

Measurements use fixed iteration counts with time-based repetition disabled. Callbacks declare whether they are asynchronous so Tinybench does not execute them to infer that property. Compiler cases use one warmup and nine measured iterations, except fresh graphs use five measured iterations and fresh or unchanged 1000-module graphs use three. Host cases use two warmups and nine measured iterations; the fresh checker case uses one warmup and five measured iterations. Source-map comparisons use one warmup and five measured iterations for each implementation. Three-sample results provide limited evidence about variance. Benchmark cases have a ten-minute timeout, including fixture preparation; ordinary tests retain Vitest's default timeout.

Initial fixture construction, correctness assertions, and fingerprint generation run outside the measured callbacks. Unchanged cases reuse a compiled session. Edit cases alternate input on every invocation, including warmup; constructing each edit is part of the measurement. CSS planning and carrier cases initialize their own sessions. Host cases use synthetic modules without a disk dependency graph and load converters before measuring reused sessions. The fresh checker case includes checker subprocess creation and closure; it reuses already loaded JavaScript modules.

To filter cases, enter `benchmarks/` and run, for example, `vp test bench compiler/compiler.bench.ts --run --mode benchmark -t "graph/single-edit"`. Configure reporters and `--outputFile` when a JSON report is needed.

The CSS-map candidate indexes mapping starts, advances source cursors and scans placeholders with a compiled pattern. It is used only by the benchmark and its colocated behavior tests. The production host still indexes source positions and searches resource replacements in `packages/host/src/css-source-map.ts`. Both benchmark variants use the current source identity and upstream-map contracts. Candidate correctness tests run in the ordinary JS test suite.
