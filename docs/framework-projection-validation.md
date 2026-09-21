# Framework projection validation

## Re-run the automated checks

Run from the repository root with the workspace lockfile installed:

```powershell
vp i --frozen-lockfile
vp run @stillwind/host#test
vp run @stillwind/integration#test:vite
$env:ASTRO_TELEMETRY_DISABLED = '1'
vp run @stillwind/integration#test:frameworks
```

The checks cover different boundaries:

| Task                                     | Evidence                                                                                                                                                                                                                   |
| ---------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `@stillwind/host#test`                   | Actual converters, native source edits, and framework compiler acceptance; Vue quotes and two script scopes, Svelte module/instance scopes, Astro server/client scripts, explicit CSS imports, and failed-edit diagnostics |
| `@stillwind/integration#test:vite`       | Raw SFC hook input, immutable CSS IDs, query subrequest exclusion, ordinary JS regression tests, and an actual build executing module CSS values and runtime helpers                                                       |
| `@stillwind/integration#test:frameworks` | Official Vue and Svelte Vite plugins and a complete Astro production build; automatic CSS contains the requested red/blue utilities, while explicitly imported green CSS remains a value until the component applies it    |

The production fixtures use Vue 3.5.42 / plugin-vue 6.0.8, Svelte 5.57.0 / vite-plugin-svelte 7.3.0, and Astro 7.3.2, as resolved in `pnpm-lock.yaml`. Projection uses the separately pinned converters in `@stillwind/projection`; Astro's projection converter is `@astrojs/compiler` 4.0.0.

Astro's own compiler is a `pre` plugin. Stillwind also sets the transform hook's `order` to `pre`, ensuring it sees the original SFC before Astro compiles it. Framework query subrequests are excluded. The official integration type declarations refer to Vite directly; the integration tests bridge that declaration boundary to Vite+ and verify the actual runtime build.

## Browser and HMR acceptance

Start all three local fixtures:

```powershell
$env:ASTRO_TELEMETRY_DISABLED = '1'
vp run @stillwind/integration#dev:frameworks
```

The command prints each fixture directory and serves Vue at `http://127.0.0.1:5174`, Svelte at `http://127.0.0.1:5175`, and Astro at `http://127.0.0.1:5176`. Each server has its own dependency cache. Stop the command with Ctrl+C. Fixtures are generated under the ignored `packages/integration/.framework-test-*` directories.

On 2026-09-10, all three pages were opened and inspected in a real browser. These observations are separate from the build assertions:

| Action                                                                    | Observed result in Vue, Svelte, and Astro                                                                                     |
| ------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Open the page                                                             | Target color `rgb(201, 22, 33)`; explicit CSS value color `rgb(17, 155, 44)`; authored `external`/`explicit` classes retained |
| Click Toggle                                                              | Target changed to `rgb(11, 44, 205)` through Vue reactivity, a Svelte rune, or Astro's client script                          |
| Edit `root.css` colors                                                    | Without manually reloading, the target changed to `rgb(140, 30, 180)` and the explicit CSS value to `rgb(230, 120, 20)`       |
| Replace the target's `style(...)` expression with `({class: 'external'})` | Generated class removed; target color became `rgb(0, 0, 0)`                                                                   |
| Restore the macro expression                                              | Generated class and `rgb(140, 30, 180)` color returned after HMR pruning                                                      |
| Replace `root.css` with invalid `@theme {`                                | Last successful target and explicit CSS colors stayed visible                                                                 |
| Restore valid CSS                                                         | Original red/green colors returned without a manual reload                                                                    |

Vue/Svelte use their HMR implementations; Astro may reload the page as part of its update handling. The assertions above do not claim preservation of component state during every kind of update.

To retain production-build fixtures for inspection, set `STILLWIND_KEEP_FRAMEWORK_FIXTURES=1` before running `test:frameworks`. The task prints successful fixture paths instead of deleting them.

## Regression and performance checks

On 2026-09-11 (JST), the final implementation passed:

- `cargo test --workspace --locked` and `cargo clippy --workspace --all-targets --locked -- -D warnings` (use `$env:CARGO_INCREMENTAL='0'` on Windows).
- `vp run @stillwind/native#check`: 7 public API tests and typechecking.
- `vp run @stillwind/host#test`: 28 tests, including original SFC edits and official compiler output.
- `vp run @stillwind/integration#test:vite`: 15 tests; `vp run @stillwind/integration#test:webpack`: 10 production builds.
- `vp run stillwind#check`: framework type entrypoints and 6 runtime/CSS tests.
- `vp run crates:check`, `vp run fixtures:check`, `vp fmt --check`, and `vp lint`.
- In `experiments/sfc-projection-research`, `vp run verify-normalization`: 32 real converter cases. `vp run verify-native` separately checks the strict raw-span mapper: 77 accepted spans and 7 expected Vue receiver rejections before normalization.

`vp run sourcemap:research` reruns the existing small/large edit benchmark, N-API JSON roundtrips, host map work, and isolated-process memory probes. The current [raw measurements](../experiments/source-map-research/results/timings.json) record the tested revision and source hashes. At 1,000 edits, the measured native map emission was 0.50 ms, its N-API JSON roundtrip 1.52 ms, and host map generation 1.24 ms. The renderer comparison at 100/1,000/5,000 edits measured 0.17/0.17/0.39 ms for the linear implementation versus 0.48/15.37/330.32 ms for repeated scanning. These component measurements do not include full framework compilation and are not application speedup claims.

The separate CSS-map research candidate used 86,236 KiB peak RSS versus 269,760 KiB for its baseline on the 2.1 MB probe. This is an experimental comparison, not a claimed production change. The map-codec/finalization replacement remains separate: additional composition still changes 29 LUB lookups, so that alternative was not adopted. `vp run sourcemap:acceptance` passed all four upstream/CSS map checks for both research variants.

Raw converter fixtures and results are excluded from formatting/linting because their bytes and offsets are evidence. Authored research scripts remain checked. Review fixtures were refreshed only for import edit endpoints; their compiled code and CSS stayed unchanged.

## Remaining scope

The host does not yet query checker type facts for projected input. The normal JS/TS checker path remains enabled. SFC source parsing, framework normalization, original-span validation, native editing, source maps, and CSS import placement are exercised independently of that deferred checker integration.

## webpack and Rspack

Run `vp run @stillwind/integration#test:webpack` for ten production builds: React, Preact, Solid, Vue, and Svelte on each bundler. All ten passed on 2026-09-10. Vue and Svelte use their official loaders. The tests verify automatic CSS ownership, explicit module CSS values, preserved external classes, and final JavaScript source maps back to the exact original macro line and column. The ordinary JS cases also execute the bundle and check emitted asset URLs in CSS values.

Unlike Vite, vue-loader can run the raw-SFC loader in query subrequests and clone it within a chain. Identical already-transformed output is skipped using loader metadata. SFC stylesheet and CSS-value requests include the loader and serialized configuration, so another worker can reconstruct them from the source file. The source receives only native import edits.

Framework loaders can discard their upstream maps. A post loader composes maps only when their embedded source content exactly matches the native-edited SFC. It reads chain-local pitch data; Rspack 2 exposes that data through `loaderItem`, while webpack exposes `data`. Both paths are exercised by the production tests. The TypeScript fixture uses SWC, which preserves upstream maps.

Browser and HMR observations above apply to Vite. These new webpack/Rspack tests do not establish Next.js/Turbopack acceptance for the explicit CSS-value feature or browser HMR for those bundlers.
