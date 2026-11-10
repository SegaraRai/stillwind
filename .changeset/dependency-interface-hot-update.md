---
"@stillwind/integration": patch
---

Apply edits of a shared module faster in the Vite development server. When an edit keeps the class names and types that a module exports, for example when it only changes utilities, the modules that import it are not compiled again. Their checker diagnostics then update when they are compiled for another reason.
