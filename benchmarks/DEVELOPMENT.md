# Benchmark development

Run one benchmark task at a time from the repository root so build and measurement
processes do not compete for CPU time.

| Task                            | Workload                                                                           |
| ------------------------------- | ---------------------------------------------------------------------------------- |
| `vp run bench:compiler`         | Native loading, compilation, graph updates, stylesheet planning, canonical queries |
| `vp run bench:compiler:wasm`    | Canonical queries through the Node WASI binding                                    |
| `vp run bench:compiler:browser` | Browser WASM canonical queries and CSS revision preparation                        |
| `vp run bench:host`             | Checker services; script, Vue, Svelte, and Astro inputs; `@plugin` graph loads     |
| `vp run bench:sorting`          | Native candidate comparisons and editor input sorting                              |
| `vp run bench:sorting:wasm`     | The same sorting workloads through Node WASI                                       |
| `vp run bench:sorting:profile`  | Identity lookup counts for ref and base input grouping                             |
| `vp run bench:source-map`       | Production CSS map generation with Unicode, CRLF, and resource replacements        |
| `vp run bench:architecture`     | CSS definition and import scale, native graphs, Vite builds and watch updates      |
| `vp run test:performance`       | Quick workloads with soft targets and hard limits                                  |
| `vp run test:performance:full`  | Full workloads including 1000 modules and 10,000 unused utility definitions        |

Vitest benchmarks use `vite-plus/test`. Benchmark and performance modes measure
native Node ESM modules with TypeScript transformation enabled. They retain raw
samples and record Git revision, Node version, platform, timestamp, and workload
settings. Reports go to ignored `benchmarks/results/`. Finite measurement runs
do not impose universal latency guarantees. Ordinary verification runs budget
configuration tests without machine-dependent timing checks.

Measurements use fixed iteration counts and disable time-based repetition.
Callbacks state whether they are asynchronous so Tinybench does not execute them
to infer that property. Fixture construction, correctness checks, and fingerprints
run outside measured callbacks unless the scenario explicitly includes them.
Three-sample cases provide limited information about variance.

To select workloads, enter `benchmarks/` and run, for example:

```sh
vp test bench compiler/compiler.bench.ts --run --mode benchmark -t "graph/single-edit"
```

## Measurement boundaries

Compiler unchanged-update cases reuse a compiled session. Edit cases alternate
input on each invocation, including warmup, and include edit construction.
Graph and stylesheet cases record output fingerprints.

Canonical CSS-revision preparation includes parsing and index preparation.
Revision reuse measures another query against the prepared snapshot. Cold-query
cases create the query outside the timer and measure one uncached candidate.
Pairwise diagnostic workloads measure complete reports separately from independent
canonical queries. Native and WASI cases run in separate processes; WASI verifies
which binding loaded. Node WASI timings do not include browser worker startup.

Host reused-service cases preload converters and use synthetic modules without
a disk dependency graph. Fresh checker cases include subprocess creation and
closure while reusing loaded JavaScript modules.

Source-map cases measure the production `cssWithSourceMap` implementation for
10 through 10,000 rules. They validate resource replacement and retained source
content, then record an output fingerprint outside the timer.

Sorting source cases query an already compiled `EditorSnapshot`. Fresh cases
start without prepared candidate keys; repeated cases reuse those keys on the
same source. Snapshot construction is measured separately. Ref and base fixtures
use syntax identities with type facts disabled. Input grouping follows
[editor sorting](../docs/standalone-analysis.md#editor-sorting).

Architecture `vite/watch-edit` cases include transform and CSS publication;
they exclude filesystem delivery and browser application. Cases marked
`watch-enabled` use the real watcher; other edit cases set `watch: null` and
check CSS contents on each request. `vite/watch-event` measures a real file write
through the watcher and hot-update hook until `stillwind:css` publication. It
excludes the browser's subsequent module request and CSS application.
`listenMs` excludes server closure; benchmark callbacks include closure.
`watchReadyMs` also waits for the watcher's initial scan. Filter cases when
investigating large graphs because their setup compiles all producers first.

See [performance budgets](performance/DEVELOPMENT.md) for thresholds and
[open work](../docs/TODO.md) for gaps in application-level measurement.
