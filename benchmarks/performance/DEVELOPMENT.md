# Performance regression checks

Run one timing task at a time after other builds and tests have finished:

- `vp run test:performance` checks canonical utility queries, 10 and 100 JS modules, 10 and 100 imported CSS files, and 100 and 1000 unused utility definitions.
- `vp run test:performance:full` also checks a palette with 1000 colors, 1000 JS modules, 1000 imported CSS files, and 10,000 unused definitions.

General measurements use three samples; sorting normally uses nine, with three for stress cases. Both commands build the release addon when needed. The full suite builds 1000 modules six times, which takes most of its time.

`vp run verify` checks the budget helper's behavior. Run the performance tasks separately to check timing on your machine.

The tests follow [Vitest's benchmark guide](https://main.vitest.dev/guide/benchmarking). Each `.bench.ts` test measures a function with `context.bench().run()` and checks its time with `expect`. Vitest uses Tinybench to measure the time.

## Budget policy

Choose limits from the operation's purpose, frequency, and amount of work. A soft target describes the desired processing time. A hard limit describes the longest processing time accepted for that workload. Measurements show whether the implementation meets those expectations. Do not set either limit by multiplying the latest measurement by a safety factor.

Interactive work needs part of the response time for transport and rendering. The general [response-time guidance](https://www.nngroup.com/articles/response-times-3-important-limits/) uses about 100ms for an immediate response, 1s for continuity, and 10s for sustained attention. The allocations below are project decisions. They apply to the measured processing stages and do not guarantee those times for a complete application.

- Ordinary document diagnostics target 25ms and must stay below 50ms for the 100-call fixture. The LSP's 40ms scheduling delay and protocol transport are outside this measurement. Generating 4950 pairwise reports has a separate 50 / 100ms budget.
- A JS edit targets 100ms from the file write to CSS publication, with a 250ms ceiling. Its transform stage receives 25 / 100ms. These limits stay the same as the number of retained modules or imported CSS files grows. The native compile stage receives 5 / 15ms even when the registry has more unused definitions.
- CSS loading rebuilds shared state for the graph. CPU work for the standard registry receives 200 / 500ms. Server startup targets 500ms, with a 1s ceiling for ordinary graphs. Loading or reloading 1000 CSS files may take up to 2s. Reloading this graph still targets 500ms; a slower result remains an optimization concern.
- A full build processes every producer and writes output. Its budget grows with the workload: 500 / 1000ms for 10 modules, 1000 / 2000ms for 100, and 5000 / 10000ms for 1000.
- Internal queries receive allocations within these operation budgets. Keep aggregate checks as well: an acceptable single-call time does not establish an acceptable time for a whole document. Apply the same operation budgets to native and WASM execution.

Soft exceedances identify work that misses the desired experience. They can remain visible across runs. Change a budget when the operation's requirements or measured boundary changes. Timing noise requires an isolated rerun and inspection of the original samples. A reproducible hard failure requires investigating the implementation and the workload before changing the limit.

## Targets

Each table cell is **soft / hard**, in milliseconds. Benchmark checks compare both values with `latency.p50`. General cases use three samples; sorting uses the sample counts listed above. Reaching or exceeding the soft target adds a `performance-soft` note, and the test continues. Reaching or exceeding the hard limit fails the test and command. Browser query checks use the percentile described below.

The test metadata records the median, maximum, and both limits. `budgets.ts` defines all limits, including those used by the browser check. Running a test does not change them.

| Canonical scenario                             | Soft / hard (ms) |
| ---------------------------------------------- | ---------------: |
| Named utility                                  |       0.01 / 0.1 |
| Arbitrary spacing, color, property, or variant |          0.1 / 1 |
| Color with a numeric opacity modifier          |          0.1 / 1 |
| CSS revision loading and index preparation     |        200 / 500 |
| 2000 cached calls                              |            2 / 5 |
| Module with 100 independent repeated colors    |          25 / 50 |
| Module with 100 independent distinct colors    |          25 / 50 |
| 4950 duplicate or conflict diagnostics         |         50 / 100 |
| Palette with 100 or 1000 colors                |          0.1 / 1 |

Canonical cases create a fresh query before each sample. The loader prepares the shared indexes when it completes the CSS revision. Candidate results and previews remain uncached in each measured query. The preparation case measures a fresh loader, CSS parsing, and index preparation together. The cached case reuses one query and measures 2000 calls.

The arbitrary-query target allocates about 10ms to 100 queries within a document's 25ms diagnosis target. The named-query target allocates a tenth of that per-call time to the early eligibility check. The 1ms arbitrary-query ceiling bounds an individual expensive lookup; the document check bounds their combined cost. The 2000 cached calls may consume up to 5ms, one tenth of the document's hard limit. Increasing the palette size keeps the same lookup budget.

Canonical module cases diagnose 100 independent style calls with one arbitrary color each. They produce no pairwise findings. Repeated and distinct colors share the document budget. Separate pairwise cases put the same 100 colors in one style and require all 4950 duplicate or conflict diagnostics, including their locations and JavaScript conversion. The pairwise hard limit is 100ms, approximately 20 microseconds per complete report. Each sample creates a new project session before the timer, so it cannot reuse compiled module results. Palette cases search for `bg-[rgb(171_205_239)]` without a matching theme value. They check that a miss stays fast as the palette grows. These measurements use the native release addon and exclude LSP transport and browser WASM execution.

`vp run bench:compiler:browser` measures the WASM binding in headless Chrome. Nine CSS revision preparations use the shared 200 / 500ms budget for their median. Each query scenario uses 20 warmup calls and 100 measured calls, with a fresh query per call. Its 95th percentile must stay below the shared 1ms hard limit; a median at or above the 0.1ms soft target adds a note. A hard exceedance fails the test after the remaining scenarios are measured. The browser report keeps every sample. It excludes worker startup, LSP transport, and editor rendering.

The standard theme cases cover the shared value index and the finite CSS index. Custom utilities that transform values or whose modifiers erase value differences can require additional candidate trials. The limits measure the listed cases rather than imposing a bound on every possible custom CSS program.

| JS modules |   Full build | Watch ready | Transform to CSS send | File write to CSS send |
| ---------- | -----------: | ----------: | --------------------: | ---------------------: |
| 10         |   500 / 1000 |  500 / 1000 |              25 / 100 |              100 / 250 |
| 100        |  1000 / 2000 |  500 / 1000 |              25 / 100 |              100 / 250 |
| 1000       | 5000 / 10000 |  500 / 1000 |              25 / 100 |              100 / 250 |

Watch startup does not compile the JS modules, so their count does not increase its budget. Use the quick suite for daily checks. It cannot show how the code performs at 1000 modules. Run the full suite after changing how sessions share data, traverse graphs, load CSS, or plan stylesheets.

| Imported CSS files | Watch ready | JS transform to CSS send | CSS file write to CSS send |
| ------------------ | ----------: | -----------------------: | -------------------------: |
| 10                 |  500 / 1000 |                 25 / 100 |                  100 / 250 |
| 100                |  500 / 1000 |                 25 / 100 |                  100 / 250 |
| 1000               |  500 / 2000 |                 25 / 100 |                 500 / 2000 |

Each CSS graph also has a root file and one JS producer. The CSS edit test changes a rule in an imported file, reloads the graph, and sends CSS. After each sample, an assertion checks that the sent CSS contains the new color. The assertion is not timed.

| Unused utility definitions | Native snapshot creation | Native edit with 100 producers |
| -------------------------- | -----------------------: | -----------------------------: |
| 100                        |                  10 / 50 |                         5 / 15 |
| 1000                       |                 50 / 200 |                         5 / 15 |
| 10,000                     |                200 / 500 |                         5 / 15 |

These cases measure two operations: creating a design-system snapshot and editing JS after the snapshot is loaded. Snapshot creation has increasing budgets for larger registries, capped at the standard registry's 200 / 500ms allocation. A JS edit retains its 5 / 15ms native compile budget at every size. The registry contains the listed number of unused utilities. Each producer has one style call with a string literal.

## Sorting targets

`bench:sorting` and `bench:sorting:wasm` also enforce these limits. They measure input grouping, ordering within utility strings, attribute name sorting, and editor snapshot construction. Grouping uses input kinds and preserves the order inside each category. Base bodies and CSS interference are not inspected.

| Sorting measurement                           | Soft / hard (ms) |
| --------------------------------------------- | ---------------: |
| 256 cached comparisons                        |            1 / 5 |
| Rank batch, at most 100 candidates            |           2 / 10 |
| Rank batch, 1000 candidates                   |          10 / 25 |
| Editor sorting, at most 100 inputs            |           5 / 10 |
| Editor sorting, 1000 inputs                   |          10 / 25 |
| Snapshot and source index, at most 100 inputs |          25 / 50 |
| Snapshot and source index, 1000 inputs        |         25 / 100 |

Ordinary sorting targets 5ms after its snapshot exists, with 25ms allocated to snapshot construction. Their hard limits total 60ms, leaving part of a 100ms response for the caller. A document with 1000 inputs is a stress case: sorting receives 10 / 25ms and construction receives 25 / 100ms. The 125ms combined ceiling leaves half of a 250ms explicit formatting operation for compilation, transport, and applying edits. Snapshot construction captures native previews, rescans sources, and binds TypeScript aliases, so its budget covers more than sorting keys.

Cached comparisons and rank batches receive smaller allocations within the sorting budget; those measurements overlap with sorting and must not be added again. These budgets exclude compilation, transport, and applying edits, so they do not establish a complete formatting response time.

Rank batches include preparation, sorting, and the binding boundary. Fresh source queries begin without candidate keys; repeated queries reuse them. Construction is measured separately and excludes compilation. The mixed cases use six categories of entries, rounded up to a multiple of six. See [editor sorting](../../docs/standalone-analysis.md#editor-sorting) for the input contract.

## Measurement boundaries

Each JS producer has eight style calls with string literals. JS cases run with a minimal registry and with the full Stillwind registry. The entry module imports and uses every producer, so the build keeps them all. Full builds include CSS generation and writing output to disk.

Full builds use three samples without warmup. Other measurements run once to warm up, then collect three samples. Tests declare whether callbacks are asynchronous and disable repetition based on elapsed time.

Watch startup measures server creation, CSS loading, listening, and the watcher's initial `ready` event. The server closes in `afterEach`, outside the measured time. Startup does not request JS modules. Starting Node and importing the measurement libraries are not timed. Vocabulary generation, dependency discovery, and source maps are disabled.

Before measuring edits, the test compiles all producers through the project API, publishes CSS once, and registers the edited module in Vite's module graph. This setup is not timed. These tests therefore do not measure publishing CSS after every initial module request.

Transform cases measure the plugin hook until it sends CSS. File edit cases write a file and wait for Vite's hot-update hook to send `stillwind:css`. `beforeEach` waits 75ms so the watcher does not merge writes from successive samples. The wait is not timed. The tests use real watcher events. They do not connect a browser or measure later JS requests and CSS application in a browser.

Each test has its own temporary directory and server. Tests run one at a time. JSON reports contain notes, raw samples, and metadata with the Git revision, Node version, platform, and measurement settings. A failed hard check also keeps the measured times and limits in its metadata. Reports are saved to `benchmarks/results/performance.json` or `performance-full.json`. Git ignores these files.

## Reference environment and maintenance

Use Windows x64 with an AMD Ryzen 7 9800X3D and the release addon as the reference environment. Record the Node and Vitest versions and keep the power mode consistent when comparing revisions. Later samples can reuse filesystem caches and loaded libraries. Three samples can catch large slowdowns, but they are not enough to compare small differences or rare slow runs. Hardware and other running programs affect timing. Check the limits on a CI runner before requiring these tests there.

If a check fails, keep the report and rerun that case on an idle machine. Use comparisons between revisions to investigate regressions within an absolute budget. To run one case, enter `benchmarks/` and use, for example: `vp test bench performance/performance.bench.ts --run --mode performance-full -t "watch/full-registry/1000"`.
