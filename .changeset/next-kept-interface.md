---
"@stillwind/integration": patch
---

Update document CSS faster after an edit in Next and Turbopack development. When an edited module keeps its exports, the modules that import it are no longer compiled again, and their checker diagnostics are refreshed when they are compiled for another reason. Next with webpack also renders the document stylesheet once for each change instead of once for each compiler and layer.
