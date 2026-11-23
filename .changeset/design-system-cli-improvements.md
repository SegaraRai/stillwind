---
"@stillwind/cli": minor
"@stillwind/host": minor
"stillwind": patch
---

`stillwind utilities` and `stillwind variants` now accept `--names-only` to list matching names without formatting CSS previews. Pattern searches in `stillwind utilities` now also include placeholder patterns for functional utilities' non-enumerable domains, such as `p-<number>` and `p-[<length-percentage>]` (`kind: "pattern"` in JSON). `stillwind theme` now includes resolved values for tokens that reference other theme variables (`resolvedValue` in JSON, and inline comments in text output).
