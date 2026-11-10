---
"@stillwind/integration": patch
---

Build Vite applications faster when pages reach source files only through dynamic imports, such as an `index.html` entry with lazy routes or Astro pages. A build now loads the source files that the project's `files` globs select into the type checker before the first compilation, so the checker no longer rebuilds its project for each newly found page.
