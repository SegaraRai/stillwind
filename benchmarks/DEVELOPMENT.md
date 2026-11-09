# Benchmark development

Run one benchmark task at a time from the repository root so build and measurement
processes do not compete for CPU time.

| Task                            | Workload                                                                            |
| ------------------------------- | ----------------------------------------------------------------------------------- |
| `vp run bench:compiler`         | Native loading, compilation, graph updates, stylesheet planning, canonical queries  |
| `vp run bench:compiler:wasm`    | Canonical queries through the Node WASI binding                                     |
| `vp run bench:compiler:browser` | Browser WASM canonical queries, CSS loading, and repeated analysis                  |
| `vp run bench:host`             | Checker services; script, Vue, Svelte, and Astro inputs; `@plugin` graph loads      |
| `vp run bench:analysis`         | Fresh CLI lint and fmt processes on the website, including process startup and exit |
| `vp run bench:sorting`          | Native candidate comparisons and editor input sorting                               |
| `vp run bench:sorting:wasm`     | The same sorting workloads through Node WASI                                        |
| `vp run bench:sorting:profile`  | Identity lookup counts for ref and base input grouping                              |
| `vp run bench:source-map`       | Production CSS map generation with Unicode, CRLF, and resource replacements         |
| `vp run bench:apps`             | Generated React Router applications with and without Stillwind                      |
| `vp run bench:architecture`     | CSS definition and import scale, native graphs, Vite builds and watch updates       |
| `vp run test:performance`       | Quick workloads with soft targets and hard limits                                   |
| `vp run test:performance:full`  | Full workloads including 1000 modules and 10,000 unused utility definitions         |

Vitest benchmarks use `vite-plus/test`. Benchmark and performance modes measure
native Node ESM modules, and Node strips TypeScript types from them. Benchmark
code must use only syntax that type stripping supports, so it cannot use `enum` or
parameter properties. Benchmark and performance modes retain raw samples and record
Git revision, Node version, platform, timestamp, and workload settings. Reports go to
ignored `benchmarks/results/`. Timings describe the chosen workloads on the current
machine. Ordinary verification checks budget settings without checking elapsed time.

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

CLI analysis runs each command three times in a new process. To select another
project and iteration count, run `node benchmarks/compiler/analysis-cli.ts <directory> <count>`
from the repository root after building the CLI.

Browser analysis measures the first lint, then repeats source edits, sorting, and
preview compilation in the same workspace. The cases cover literal inputs and ref
unions checked against Stillwind's built declarations. Measurements start after
the WASM binding loads and the workspace is created. They exclude LSP transport,
debounce, UI rendering, and Prettier.

Run `vp run @stillwind-internal/benchmarks#bench:play-memory` to measure the complete
playground in installed Chrome. Each preset runs three times in a fresh browser.
Samples follow startup, five source edits, a CSS edit, a configuration edit, and
recovery after terminating the application build worker. The WASM owner workers
collect garbage before reporting their initial and current linear memory sizes.
On Windows, the report also sums private committed bytes and working sets across
the browser's processes. Working sets can count shared pages more than once.
These samples describe completed stages and do not capture every transient peak.

`STILLWIND_PLAY_MEMORY_RUNS` changes the repetition count, and
`STILLWIND_PLAY_MEMORY_PRESETS` selects comma-separated preset IDs.
`STILLWIND_PLAY_TEST_DIRECTORY` selects an existing static build.
`STILLWIND_PLAY_MEMORY_OUTPUT` selects the JSON report path; the default is
`benchmarks/results/playground-memory.json`. Keep the Chrome version, hardware,
workload, and collection settings the same when comparing builds.

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

## Generated applications

`bench:apps` generates React Router applications in three variants and measures them
with the same workload. The `stillwind` variant uses the Vite integration. The `plain`
variant replaces `style()` with a local function that joins class names and emits no
CSS, so the difference from `plain` is the cost of Stillwind. The `tailwind` variant uses
the same local function and generates CSS with `@tailwindcss/vite`, so it shows the cost
of the usual alternative. Tailwind CSS does not read the object form of `style()`, so its
variant generates no CSS for the `hover:` and `disabled:` groups. The applications
resolve their dependencies from `e2e/frameworks/vite8-react19-reactrouter8` and are
written to its ignored `.e2e/` directory. They import the Tailwind CSS packages of this
benchmark package by absolute path, because that fixture does not install them.

Each run measures:

- a production build after removing the previous output and Vite's dependency cache;
- development server startup until the first successful SSR response, also without
  Vite's dependency cache;
- the first page load and a reload until hydration, in installed Chrome;
- three edits until the page shows them: a component that only its chain imports, the
  shared style module that every module imports, and a theme token in the CSS entrypoint.
  The `plain` variant waits for the changed class name and skips the theme edit. The
  other variants wait for the computed style.

Sizes describe the module graph. `small` has 8 routes and 24 components, `medium` has
30 routes and 240 components, and `large` has 100 routes and 1200 components. Every
module imports the shared styles through a barrel module, and component exports compose
the exported style of the next component in their chain. Set these variables to select
workloads:

| Variable                        | Default                    | Meaning                               |
| ------------------------------- | -------------------------- | ------------------------------------- |
| `STILLWIND_BENCH_APPS_SIZES`    | `small,medium`             | Comma-separated sizes                 |
| `STILLWIND_BENCH_APPS_VARIANTS` | `plain,tailwind,stillwind` | Comma-separated variants              |
| `STILLWIND_BENCH_APPS_RUNS`     | `3`                        | Runs for each size and variant        |
| `STILLWIND_BENCH_APPS_KEEP`     | unset                      | `1` keeps the generated applications  |
| `STILLWIND_BENCH_APPS_OUTPUT`   | `results/apps.json`        | Report path, relative to `benchmarks` |

Every Node process that a measured command starts loads `apps/preload.mjs`. It records
resource usage and the phases that `@stillwind/host` publishes on the
`stillwind:host:phase` diagnostics channel. The report sums phases over processes, and
the console shows them for the last run, both for the whole command and for each edit
with its restoration. Phases whose names start with `checker-` describe work inside the
TypeScript checker. Asynchronous phases include waiting time, so
their totals can exceed elapsed time when Vite transforms modules concurrently. Phase
counts show repeated work directly: for example, more `scan` calls than modules mean
that the host scanned the same source for several entries.

See [performance budgets](performance/DEVELOPMENT.md) for thresholds and
[open work](../docs/TODO.md) for gaps in application-level measurement.
