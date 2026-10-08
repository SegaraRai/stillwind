# Ownership cleanup: canonical preparation and diagnostics

The cleanup keeps immutable identities in `LeanStr`, local evaluation records in
their owning context, and synchronization only where queries or snapshots share
state across threads. The representation design is fixed in
[ownership and string design](ownership-and-string-design.md).

## Work and contracts

Canonical module measurements contain 100 independent style calls. Each call has
one arbitrary color, so the module produces no pairwise findings. The existing
15ms repeated-value and 30ms distinct-value hard limits apply to these cases.

The separate diagnostic cases contain one style with 100 colors. They require all
4950 duplicate or conflict reports. No lint is disabled. Each report retains its
message, severity, primary and related locations, notes, actions, and ordering.
These cases have a 100ms hard limit, about 20 microseconds per complete report,
including the JavaScript conversion. Their soft target is 50ms. This is a distinct
output workload; the canonical limits and the 300ms CSS preparation limit remain
unchanged.

## Changes supported by profiling

CSS loading parses definitions, builds the theme, and prepares canonical indexes.
The baseline template comparison included token-use provenance in full utility
equality. Different token names rejected otherwise identical emitted structures,
which caused unnecessary finite candidate previews. Template matching now compares
all emitted fields after substitution. It retains ordering, property requirements,
selectors, nested rules, additional targets, and declaration comparisons. Source
use records stay attached to their original resolutions. CSS differences still
use the finite fallback. The exhaustive utility destructuring requires review when
the utility gains a field.

Pairwise conflict checks now group direct declarations by source use once per
style. Each pair inspects those declarations instead of scanning all supplies.
Conditions, contexts, shorthand policy, and report order retain their contracts.

Each `Reporter` retains source facts and resolved ranges until it finishes.
Source revisions and lint directives are computed once per source allocation.
Repeated labels reuse coordinates. The retained `Arc<SourceFile>` prevents reuse
of a pointer used as a lookup key. Separate source registries can reuse a file ID
without sharing entries. Failed reports do not commit revisions or diagnostics.
The transient diagnosis batch transfers owned reports into the native output;
retained project batches still use the borrowed conversion.

## Serial comparison

The baseline is `f2bd5a59`. Both addons use the release profile and
`x86_64-pc-windows-msvc`, with `CARGO_INCREMENTAL=0`. The machine has an AMD Ryzen 7
9800X3D and runs Windows x64 with Node 24.21.0. Each operation has one warmup and
nine samples. Baseline and final measurements run serially, with no overlapping
builds or tests. Explicit garbage collection and one event-loop turn occur outside
each sample in both runs. Input construction and output hashing are outside the
timer.

| Operation                       | Before median, ms | After median, ms |
| ------------------------------- | ----------------: | ---------------: |
| CSS revision preparation        |           510.807 |          266.835 |
| Cold spacing                    |             0.195 |            0.170 |
| Cold matching color             |             0.191 |            0.182 |
| Cold missing color              |             0.144 |            0.143 |
| 4950 duplicate reports          |           113.033 |           44.240 |
| 4950 conflict reports           |           147.326 |           58.224 |
| 100 independent repeated colors |             4.318 |            4.422 |
| 100 independent distinct colors |            12.316 |           10.833 |

All input hashes and complete output hashes match. The independent repeated case
has a small increase, so these results do not establish an improvement for every
operation. The earlier ownership measurements used different concurrency and
cannot isolate the effect of the representation changes.

The isolated instrumented addon records one warmup and one sample. Its nested
stages overlap and must not be added together. These timings identify work; the
uninstrumented nine-sample runs above measure the overall change.

| Profile stage                               |           Before, ms |          After, ms |
| ------------------------------------------- | -------------------: | -----------------: |
| CSS definition parsing                      |               35.421 |             35.661 |
| Theme preparation                           |                4.310 |              3.730 |
| Canonical index preparation                 |              487.139 |            208.699 |
| Candidate previews                          | 257.637 / 8532 calls | 14.535 / 928 calls |
| Duplicate diagnosis: source scan            |                0.101 |              0.079 |
| Duplicate diagnosis: analysis finalization  |                0.035 |              0.017 |
| Duplicate diagnosis: compiler               |                4.948 |              3.873 |
| Duplicate diagnosis: pairwise checking      |                2.150 |              2.216 |
| Duplicate diagnosis: report construction    |               62.930 |              7.604 |
| Duplicate diagnosis: native output structs  |                3.472 |              1.656 |
| Conflict diagnosis: pairwise checking       |               28.993 |              7.354 |
| Conflict diagnosis: property overlap checks |               23.295 |              1.059 |
| Conflict diagnosis: report construction     |               69.757 |             13.752 |
| Conflict diagnosis: native output structs   |                4.252 |              3.436 |

The baseline probe also records template rejection reasons; 7639 comparisons reject token-use metadata. Its additional counters make the stage timings approximate.

Coordinate calculations fall from 19,800 to 200 calls per operation, and lint
directive parsing falls from 4953 to 4 calls. Each operation still resolves 9900
labels and produces 4950 reports. The synchronous Node timer includes JavaScript
output conversion; the Rust root timer ends before that conversion. Their
difference also includes instrumentation overhead and is not an exact isolated
conversion measurement.

## Reproduction and evidence

[The evidence](../benchmarks/compiler/ownership-performance-evidence.json) keeps
every sample, input and output hashes, addon hashes, and instrumented stage records.
Run timings individually after all builds and tests finish:

```powershell
node --expose-gc benchmarks/research/canonical-profile.mjs <addon.node> <result.json>
```

Build the baseline and final addons with the same Cargo target and release flags,
and copy each artifact to a separate path before comparing them. For profiling,
the following script archives the selected revision into a fresh repository-local
directory. `--working-tree` overlays tracked Rust and Cargo files. It inserts
timers only into that isolated tree and checks each insertion anchor.

```powershell
python benchmarks/research/profile-canonical.py --revision f2bd5a59 --directory target/profile-before
python benchmarks/research/profile-canonical.py --working-tree --directory target/profile-after
$env:CARGO_INCREMENTAL='0'
cargo build --manifest-path target/profile-after/Cargo.toml -p stillwind-napi --release --target x86_64-pc-windows-msvc
Copy-Item target/profile-after/target/x86_64-pc-windows-msvc/release/stillwind_napi.dll target/profile-after.node
node benchmarks/research/canonical-profile.mjs target/profile-after.node target/profile-after.json profile
```

Apply the same build and copy steps to the baseline tree. Profiling writes stage
records to stderr. Use uninstrumented addons for acceptance timings.

## Acceptance

`vp run verify` passes workspace Rust tests, strict Clippy, formatting, TypeScript,
generated-data checks, fixture comparisons, and 970 JavaScript tests. The rebuilt
WASM binding passes 47 tests in a separate process. The native compiler benchmark
passes 46 cases, and the WASM benchmark passes 19 cases. Their measured CSS
preparation medians are 259.604ms and 422.734ms respectively. The performance
regression suite applies its limits to the native reference target.

The final `vp run test:performance:full` passes all 242 cases, including 208 sorting
cases. Its medians are 290.236ms for CSS preparation, 3.944ms and 8.062ms for the
independent canonical modules, and 27.124ms and 32.409ms for the complete pairwise
reports. All original hard limits retain their values. The CSS preparation maximum
is 307.321ms; acceptance uses the median of three samples.

The first full attempt, before the final template comparison, passes 240 of 242
cases. CSS preparation exceeds 300ms and watch startup with 1000 CSS imports
exceeds 2000ms. A focused final-code run passes CSS preparation at 246.335ms but
fails watch startup at 2035.877ms. A subsequent focused watch run passes; the final
full run also passes startup at 571.720ms, transformation at 6.347ms, and CSS file
edit publication at 248.417ms. No host or watcher code changes between these final
runs. Their startup variation is not attributed to a production optimization.
The evidence retains these earlier observations as well as the passing full run.
Filesystem caches and event timing remain sources of uncertainty. Check the
reference limits on an idle CI runner before requiring them there.
