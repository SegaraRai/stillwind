---
"stillwind": patch
"@stillwind/cli": patch
---

Analysis no longer prepares the TypeScript checker again each time CSS ordering reads a module that the analyzed files do not import for type information. `stillwind lint`, `fmt`, and `check` and the editor finish sooner in projects whose components are imported through other modules.
