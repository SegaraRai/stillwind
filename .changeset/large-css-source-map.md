---
"@stillwind/integration": patch
---

Fix document CSS in large Next applications that use webpack. After an edit, the stylesheet failed with "Maximum call stack size exceeded" while its source map was read, so the page returned an error and the edit never applied.
