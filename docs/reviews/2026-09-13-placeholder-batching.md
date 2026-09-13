# Placeholder batching

## Implementation

- `stillwind_css::find_css_functions` indexes multiple exact, unescaped function names in one CSS token walk. Results include source ranges and nested matches. `find_css_function` delegates to it.
- `expand_placeholders` shares that index across absent families, expands present families in the supplied order, and refreshes positions after replacements. `expand_placeholder` delegates to it. Work remains proportional to the number of present families rather than repeated whole-input searches for each occurrence.
- Resolution keeps the existing family order and the declaration-removal boundary at `--modifier-none()`. Recursive expansion still rejects cycles and enforces per-family depth/work/output limits. Batch failure leaves the input intact.
- Final unresolved-marker validation uses one batch lookup. Calc folding, theme-reference substitution and nested `var()` fallback formatting also consume batch matches instead of restarting scans.
- Lookup skips tokenization when there is no opening parenthesis, or when all requested names start with `--` and the source contains no `--`. Ordinary function tokens bypass name-list comparisons in the latter mode. Strings, comments, escaped names and URL contents retain their lexical handling.

## Measurement

Command: `cargo run --release -p stillwind-design-system --example placeholder_resolution`.
The example measures public `resolve_utility` calls against a pre-parsed theme, with five samples of 2,000 calls per case and reports the median. Both runs used the same local machine and `CARGO_INCREMENTAL=0`; the baseline includes the preceding correctness fixes and predates this batching change.

| Case                               | Before (ns/call) | After (ns/call) | Speedup |
| ---------------------------------- | ---------------: | --------------: | ------: |
| No placeholders                    |            2,243 |             700 |   3.20x |
| One `--value`                      |            3,439 |           1,845 |   1.86x |
| 32 `--value` calls in `calc()`     |          130,560 |          18,493 |   7.06x |
| `--value`, `--modifier`, `--alpha` |            7,739 |           4,420 |   1.75x |

These are focused local microbenchmark results, not end-to-end build speedups. The checked-in example allows the same cases to be rerun independently.

## Regression coverage

New tests exercise ordered family expansion, functions introduced by replacements, no revisiting earlier families, transactional failure, consuming nested matches with an outer replacement, source ranges for unclosed outer calls, lexical context before a scan offset, duplicate lookup names, and the modifier declaration-removal boundary. Existing cycle/amplification tests remain in place.

Validation: `cargo test --workspace` passed (463 passed, 0 failed, 1 intentionally ignored registry audit); `cargo clippy --workspace --all-targets -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, `cargo fmt --all -- --check`, and `git diff --check` passed.
