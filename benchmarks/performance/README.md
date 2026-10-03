# Performance regression checks

Run one timing task at a time after other builds and tests have finished:

- `vp run verify:performance` checks 10 and 100 JS modules, 10 and 100 imported CSS files, and 100 and 1000 unused utility definitions.
- `vp run verify:performance:full` also checks 1000 JS modules, 1000 imported CSS files, and 10,000 unused definitions.

The quick suite has 12 tests and takes about six seconds on the machine listed below. The full suite has 18 tests and takes about 40 seconds. Each measurement uses three samples. These times exclude builds needed before the tests. Both commands build the release addon when needed. The full suite builds 1000 modules six times, which takes most of its time.

`vp run verify` checks the budget helper's behavior. Run the performance tasks separately to check timing on your machine.

The tests follow [Vitest's benchmark guide](https://main.vitest.dev/guide/benchmarking). Each `.bench.ts` test measures a function with `context.bench().run()` and checks its time with `expect`. Vitest uses Tinybench to measure the time.

## Targets

Each table cell is **soft / hard**, in milliseconds. The tests compare both values with `latency.p50`, the median of three samples. Reaching or exceeding the soft target adds a `performance-soft` note, and the test continues. Reaching or exceeding the hard limit fails the test and command.

The test metadata records the median, maximum, and both limits. `budgets.ts` defines the limits. Running a test does not change them.

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
