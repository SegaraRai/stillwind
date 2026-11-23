---
"@stillwind/cli": minor
"@stillwind/analysis": minor
"stillwind": patch
---

Added `stillwind check` and `stillwind check --fix` to run lint checks and utility sorting in a single analysis pass. `stillwind check` reports lint findings and files that need sorting without saving, and exits with `1` when errors or unsorted files remain. `stillwind check --fix` applies converged automatic fixes and utility sorting, saves changed files, and reports findings for the final sources. `@stillwind/analysis/node` exports the corresponding `check()` function, and `stillwind`'s agent instructions now recommend `stillwind check --fix`.
