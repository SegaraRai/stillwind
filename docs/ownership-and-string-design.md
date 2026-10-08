# Ownership and string representation

Finite compiler decisions use enums. Parsers accept text at their boundaries and
return typed values after validation. Serialization preserves existing JSON values,
CSS spelling, generated names, and naming callback arguments.

Immutable snapshots, source revisions, project transactions, and queries that move
between threads retain their shared ownership. Evaluation records belong to one
evaluation and never become part of an immutable theme. JavaScript naming callbacks
use single-threaded shared ownership.

Immutable host identities, generated and registered names, parsed candidates, and
canonical signatures use `LeanStr`. Output buffers that append or replace text use
`String`. Borrowed source slices and externally supplied open identifiers use `&str`.
Conversion to JavaScript strings happens at the binding boundary.

## Implementation phases

1. Type generated symbol kinds, locator projections, container types, lint detection
   keys, canonical spacing units, viewport axes, and artifact adapter keys. Update
   binding declarations and callers while preserving their serialized values.
2. Store canonical labels directly in signature groups. Remove records and previews
   that no query reads after preparation. Normalize temporary templates without a
   preview cache. Reuse the analysis source in lint edits.
3. Move token and arbitrary-value recording into an evaluation context that borrows
   the immutable theme. Consume completed records without cloning their collections.
4. Use `Rc` for naming callbacks and generator sets. Preserve callback lifetime,
   pointer identity, memoization, and transaction behavior.
5. Adopt `LeanStr` for source identities, generated names, binding names, registry
   names, candidates, and canonical signatures where values remain immutable.
   Retain `String` for builders and mutable CSS or JavaScript output.

Each implementation phase is validated and committed independently. Final acceptance
includes the workspace check and test tasks, shared native/WASM behavior tests, and
native/WASM compiler benchmarks. Measurements compare the same benchmark cases and
record the runtime, representation limits, and any observed regressions.

## Representation boundaries

`lean_string` 0.7 provides `LeanStr`, which stores an immutable string in two machine
words. Its inline storage holds up to 16 UTF-8 bytes on the native 64-bit target and
8 bytes on WASM32. Longer values share a reference-counted allocation when cloned.
The heap representation has no spare capacity. The [upstream documentation](https://github.com/ryota2357/lean_string)
describes these limits and the separate heap layouts of `LeanStr` and `LeanString`.

`LeanString` is not used for growing output buffers. CSS and JavaScript builders
append and replace text, and conversion between the two Lean types may copy or
reallocate a heap buffer. Their output remains `String`; immutable keys convert
once after construction.

Finite generated symbol kinds and locator projections become strings at the N-API
boundary. Their TypeScript fields expose the permitted values as unions. Container
types preserve CSS spelling, including the canonical order of combined keywords.
Open CSS property names, module paths, frontend names, and authored source text
remain text rather than enums.

## Shared ownership retained

| Owner                                                              | Reason for sharing or synchronization                                                                                                                          |
| ------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Source files and source text                                       | Diagnostics, transforms, analysis, and source maps retain the same immutable source revision.                                                                  |
| Theme, snapshot, and definition registries                         | Queries and compiled artifacts share immutable definitions and prepared lookup data.                                                                           |
| Project analyses, artifact keys, artifacts, and dependency indexes | Cloned project state shares accepted data while a transaction prepares its replacement.                                                                        |
| Snapshot analysis cache                                            | Queries on different threads share analyses. The mutex protects entries, and each entry's `OnceLock` initializes its analysis once.                            |
| Cached canonical previews                                          | Cloned queries can reuse previews on another thread. The mutex protects normalized signatures. Temporary template normalization uses a separate pure function. |
| Style resolver registry                                            | Returned styles retain the registry independently of the resolver and can move between threads.                                                                |
| Lightning CSS warning collector                                    | The parser's warning API requires `Arc<RwLock<_>>`; the minifier returns the collected warnings.                                                               |

Naming callbacks use `Rc` because a compilation calls JavaScript synchronously on
its owning thread. Evaluation contexts borrow the theme and own local selection
records. Neither operation requires an atomic reference count or a lock.

## Acceptance and measurements

The workspace verification passed Rust tests, strict Clippy, fixture comparisons,
generated-data checks, TypeScript checks, formatting, linting, and 970 JavaScript
tests. Native binding contracts passed 42 tests. The separate WASM process passed
47 tests. Shared naming tests cover Unicode identities, strings below and above the
inline storage limits, and callbacks retained across project session calls.

The native compiler benchmark passed 44 tests and recorded 43 measured cases. The
WASM compiler benchmark passed 17 tests and recorded 16 measured cases. The module
cases check all 4950 pairwise duplicate or conflict diagnostics for 100 uses.

[The measurement evidence](../benchmarks/compiler/ownership-evidence.json) records
every case, all samples, the runtime, and the revision for both runs. Native
and WASM baseline tasks ran concurrently; final tasks ran serially. These runs do
not isolate the effect of the implementation. The baseline's two module assertions
expected empty diagnostics and failed after collecting their samples.

Selected means in milliseconds are shown below. Each value includes the margin
reported by the benchmark runner.

| Case                            |    Native before |     Native after |      WASM before |       WASM after |
| ------------------------------- | ---------------: | ---------------: | ---------------: | ---------------: |
| Cold spacing query              |    0.049 ± 0.005 |    0.087 ± 0.008 |    0.094 ± 0.018 |    0.088 ± 0.017 |
| CSS revision preparation        | 501.403 ± 21.765 | 451.443 ± 10.760 | 904.817 ± 17.339 | 805.793 ± 30.651 |
| 2000 cached queries             |    0.912 ± 0.138 |    0.775 ± 0.014 |    5.523 ± 0.666 |    5.119 ± 0.665 |
| Module with 100 repeated values |   89.948 ± 2.533 |   81.072 ± 2.670 |  284.125 ± 4.831 | 232.004 ± 10.255 |
| Module with 100 unique values   |  108.896 ± 3.089 |  105.026 ± 4.926 | 279.333 ± 16.422 | 269.911 ± 18.631 |

Three native cases have higher means with margins that do not overlap the baseline:
the cold spacing query, a single edit in a graph with 300 modules
(1.722 ± 0.108 to 2.281 ± 0.413 ms), and CSS carrier generation for 300 modules
(0.216 ± 0.036 to 0.268 ± 0.012 ms). WASM has no case with that pattern. The native
increases remain observations subject to the different measurement conditions;
these results do not establish a universal performance improvement.

The full performance suite passed 237 of 240 tests, including all 208 sorting
tests. Three canonical cases exceeded the existing hard limits. The suite rebuilt
the addon from a working tree that also contained separate uncommitted import
cleanup. The performance checks retained their existing limits.

| Case                            | Baseline median, ms | Full performance median, ms | Hard limit, ms |
| ------------------------------- | ------------------: | --------------------------: | -------------: |
| CSS revision preparation        |             495.486 |                     452.882 |            300 |
| Module with 100 repeated values |              90.867 |                      89.845 |             15 |
| Module with 100 unique values   |             109.401 |                     116.373 |             30 |

The baseline compiler measurements also exceeded these limits. They use nine
samples, while the full performance cases use three. This cleanup therefore does
not establish that the performance release gate passes. The timing and workload
need a separate investigation before using this gate as release acceptance.

The subsequent [performance investigation](ownership-performance-2026-10-08.md) compares the accepted cleanup against targeted optimizations under matching serial conditions. It separates 100 independent canonical uses from 4950 complete pairwise diagnostic reports and retains both workloads.
