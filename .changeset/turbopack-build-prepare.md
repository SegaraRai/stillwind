---
"@stillwind/integration": patch
---

Build Next applications with Turbopack faster. A production build now loads the entries and the source files that the project's `files` globs select into one type checker that the loader and document CSS compilers of a worker process share, so the checker no longer rebuilds its project for each compiled module.
