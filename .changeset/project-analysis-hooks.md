---
"@stillwind/config": minor
"@stillwind/analysis": minor
"@stillwind/cli": minor
"stillwind": minor
---

Added project-scoped `hooks` (`defineHook` and `AnalysisHook` from `stillwind/config` and `@stillwind/config`) for in-memory formatting in `stillwind fmt` and `stillwind check`. Hooks support `setup`, `teardown`, and per-file `format` callbacks so formatters such as Oxfmt run after Stillwind utility sorting in one pass, including `--check`. `stillwind info` now also lists configured hook names.
