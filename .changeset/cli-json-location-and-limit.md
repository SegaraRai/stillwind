---
"@stillwind/cli": minor
---

Add `--limit <n>` to `stillwind check` and `stillwind lint` to cap displayed diagnostics while preserving full counts and exit codes, add `summary.byCode` and `summary.omittedDiagnostics` to `--format json`, and expose working-directory-relative `file` paths with 1-based `line` and `column` fields directly on JSON diagnostics and inspect previews.
