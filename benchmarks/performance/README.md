# Performance regression checks

Run one timing task at a time after other builds and tests have finished:

- `vp run test:performance` checks canonical utility queries, 10 and 100 JS modules, 10 and 100 imported CSS files, and 100 and 1000 unused utility definitions.
- `vp run test:performance:full` also checks a palette with 1000 colors, 1000 JS modules, 1000 imported CSS files, and 10,000 unused definitions.

The quick suite has 27 general cases plus 208 sorting cases. The full suite has 34 general cases plus the same sorting cases. General measurements use three samples; sorting normally uses nine, with three for stress cases. Both commands build the release addon when needed. The full suite builds 1000 modules six times, which takes most of its time.

`vp run verify` checks the budget helper's behavior. Run the performance tasks separately to check timing on your machine.

The tests follow [Vitest's benchmark guide](https://main.vitest.dev/guide/benchmarking). Each `.bench.ts` test measures a function with `context.bench().run()` and checks its time with `expect`. Vitest uses Tinybench to measure the time.

## Targets

Each table cell is **soft / hard**, in milliseconds. The tests compare both values with `latency.p50`, the median of three samples. Reaching or exceeding the soft target adds a `performance-soft` note, and the test continues. Reaching or exceeding the hard limit fails the test and command.

The test metadata records the median, maximum, and both limits. `budgets.ts` defines the limits. Running a test does not change them.

| Canonical scenario                             | Soft / hard (ms) |
| ---------------------------------------------- | ---------------: |
| Named utility                                  |       0.01 / 0.1 |
| Arbitrary spacing, color, property, or variant |          0.1 / 1 |
| Color with a numeric opacity modifier          |          0.1 / 1 |
| CSS revision loading and index preparation     |        200 / 300 |
| 2000 cached calls                              |            1 / 5 |
| Module with 100 independent repeated colors    |           5 / 15 |
| Module with 100 independent distinct colors    |          15 / 30 |
| 4950 duplicate or conflict diagnostics         |         50 / 100 |
| Palette with 100 or 1000 colors                |          0.1 / 1 |

Canonical cases create a fresh query before each sample. The loader prepares the shared indexes when it completes the CSS revision. Candidate results and previews remain uncached in each measured query. The preparation case measures a fresh loader, CSS parsing, and index preparation together. The cached case reuses one query and measures 2000 calls.

Canonical module cases diagnose 100 independent style calls with one arbitrary color each. They produce no pairwise findings. Separate pairwise cases put the same 100 colors in one style and require all 4950 duplicate or conflict diagnostics, including their locations and JavaScript conversion. The pairwise hard limit is 100ms, approximately 20 microseconds per complete report; the canonical limits remain 15ms and 30ms. Each sample creates a new project session before the timer, so it cannot reuse compiled module results. Palette cases search for `bg-[rgb(171_205_239)]` without a matching theme value. They check that a miss stays fast as the palette grows. These measurements use the native release addon and exclude LSP transport and browser WASM execution.

`vp run bench:compiler:browser` measures the WASM binding in headless Chrome. It checks the median of nine CSS revision preparations against 300ms. Each query scenario uses 20 warmup calls and 100 measured calls, with a fresh query per call. Its 95th percentile must stay below 1ms; a median at or above 0.1ms adds a note. The browser report keeps every sample. It excludes worker startup, LSP transport, and editor rendering.

The standard theme cases cover the shared value index and the finite CSS index. Custom utilities that transform values or whose modifiers erase value differences can require additional candidate trials. The limits measure the listed cases rather than imposing a bound on every possible custom CSS program.

| JS modules |   Full build | Watch ready | Transform to CSS send | File write to CSS send |
| ---------- | -----------: | ----------: | --------------------: | ---------------------: |
| 10         |    250 / 750 |  300 / 1000 |               10 / 40 |               50 / 150 |
| 100        |  1000 / 2000 |  500 / 1000 |               30 / 80 |               75 / 200 |
| 1000       | 5000 / 10000 | 1000 / 2000 |             100 / 200 |              100 / 250 |

The hard limits allow for variation between runs. Use the quick suite for daily checks. It cannot show how the code performs at 1000 modules. Run the full suite after changing how sessions share data, traverse graphs, load CSS, or plan stylesheets.

| Imported CSS files | Watch ready | JS transform to CSS send | CSS file write to CSS send |
| ------------------ | ----------: | -----------------------: | -------------------------: |
| 10                 |   100 / 300 |                  10 / 30 |                   50 / 150 |
| 100                |   300 / 750 |                  20 / 50 |                  150 / 400 |
| 1000               | 1000 / 2000 |                100 / 200 |                1000 / 2000 |

Each CSS graph also has a root file and one JS producer. The CSS edit test changes a rule in an imported file, reloads the graph, and sends CSS. After each sample, an assertion checks that the sent CSS contains the new color. The assertion is not timed.

| Unused utility definitions | Native snapshot creation | Native edit with 100 producers |
| -------------------------- | -----------------------: | -----------------------------: |
| 100                        |                  10 / 50 |                         5 / 15 |
| 1000                       |                 50 / 200 |                         5 / 15 |
| 10,000                     |               300 / 1000 |                         5 / 15 |

These cases measure two operations: creating a design-system snapshot and editing JS after the snapshot is loaded. The registry contains the listed number of unused utilities. Each producer has one style call with a string literal.

## Sorting targets

`bench:sorting` and `bench:sorting:wasm` also enforce these limits. They measure input grouping, ordering within utility strings, attribute name sorting, and editor snapshot construction. Grouping uses input kinds and preserves the order inside each category. Base bodies and CSS interference are not inspected.

| Sorting measurement                    | Soft / hard (ms) |
| -------------------------------------- | ---------------: |
| 256 cached comparisons                 |      2.56 / 25.6 |
| Rank batch, at most 100 candidates     |           2 / 10 |
| Rank batch, 1000 candidates            |         20 / 100 |
| Editor sorting, at most 100 inputs     |            1 / 5 |
| Editor sorting, 1000 inputs            |          10 / 30 |
| Snapshot and source index construction |         30 / 300 |

The comparison limits correspond to 0.01 / 0.1ms per call. Rank batches include preparation, sorting, and the binding boundary. Fresh source queries begin without candidate keys; repeated queries reuse them. Construction is measured separately and excludes compilation. The mixed cases use six categories of entries, rounded up to a multiple of six. See [the sorting report](../../docs/sorting-optimization-2026-10-07.md) for results and limits.

## Measurement boundaries

Each JS producer has eight style calls with string literals. JS cases run with a minimal registry and with the full Stillwind registry. The entry module imports and uses every producer, so the build keeps them all. Full builds include CSS generation and writing output to disk.

Full builds use three samples without warmup. Other measurements run once to warm up, then collect three samples. Tests declare whether callbacks are asynchronous and disable repetition based on elapsed time.

Watch startup measures server creation, CSS loading, listening, and the watcher's initial `ready` event. The server closes in `afterEach`, outside the measured time. Startup does not request JS modules. Starting Node and importing the measurement libraries are not timed. Vocabulary generation, dependency discovery, and source maps are disabled.

Before measuring edits, the test compiles all producers through the project API, publishes CSS once, and registers the edited module in Vite's module graph. This setup is not timed. These tests therefore do not measure publishing CSS after every initial module request.

Transform cases measure the plugin hook until it sends CSS. File edit cases write a file and wait for Vite's hot-update hook to send `stillwind:css`. `beforeEach` waits 75ms so the watcher does not merge writes from successive samples. The wait is not timed. The tests use real watcher events. They do not connect a browser or measure later JS requests and CSS application in a browser.

Each test has its own temporary directory and server. Tests run one at a time. JSON reports contain notes, raw samples, and metadata with the Git revision, Node version, platform, and measurement settings. A failed hard check also keeps the measured times and limits in its metadata. Reports are saved to `benchmarks/results/performance.json` or `performance-full.json`. Git ignores these files.

## Reference environment and maintenance

The limits were checked on Windows x64 with an AMD Ryzen 7 9800X3D, Node 24.21, Vitest 5.0.1, and the native release addon. Later samples can reuse filesystem caches and loaded libraries. Three samples can catch large slowdowns, but they are not enough to compare small differences or rare slow runs. Hardware and other running programs affect timing. Check the limits on a CI runner before requiring these tests there.

If a check fails, inspect the samples and rerun that case on an idle machine. Fix a slowdown caused by the code. Change a limit only after reviewing why it no longer fits the environment or workload. To run one case, enter `benchmarks/` and use, for example: `vp test bench performance/performance.bench.ts --run --mode performance-full -t "watch/full-registry/1000"`.
