---
"@stillwind/integration": patch
---

Vite development servers now apply edits of style modules that an editor or tool saves by truncating the file and then writing it. Before this fix, a hot update on Linux could read the file while it was empty, and the edited styles stayed missing until the next edit.
