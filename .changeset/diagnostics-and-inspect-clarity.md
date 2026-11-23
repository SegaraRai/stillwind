---
"@stillwind/cli": minor
"@stillwind/analysis": minor
"stillwind": patch
---

Diagnostic actions in CLI output now distinguish automatic fixes from manual suggestions (`actions[].autofix`, `summary.autofixable`, and `(autofix)` / `(suggestion)` labels in text output), and the `--fix` hint is shown only when automatic fixes are available. `stillwind inspect` now explains why no previews were returned (`emptyReason`: `no-styles`, `line-out-of-range`, `column-out-of-range`, or `no-match-at-position`). Uncompiled runtime placeholder errors for every `style.*` macro now include guidance on build integration ordering and `docs/integrations.md`.
