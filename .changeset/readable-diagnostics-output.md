---
"@stillwind/cli": patch
"@stillwind/integration": patch
"stillwind": patch
---

Make CLI and build output easier to read. `stillwind --help` now highlights headings and options, lists `--format files` only for `stillwind fmt`, and suggests valid commands and `--help` for an unknown command or option. Text output colors file paths, codes, `note:`, `help:`, and `docs:` labels, shows `[project N]` only when results come from several projects, and gives `stillwind inspect` diagnostics the same colors and docs links. Compiler and lint messages no longer show internal byte offsets or debug values, name the conflicting framework and source format for `SW02001`, show the rejected prefix for `SW01001`, and use consistent punctuation and backticks. Unterminated blocks, nesting overflows, and syntax errors in CSS now point at the failing character instead of the whole stylesheet. When the failing CSS has no document URI, such as CSS that a plugin generates, the error message names that source and the byte offset. Vite reports compile errors in the error overlay with a clickable location and a code frame, and build warnings use the `stillwind: <rule>: <message> (<path>:<line>:<column>)` form.
