---
"@stillwind/integration": patch
---

In development, pages that link `/__stillwind/style.css?environment=ssr` now reload that stylesheet when the CSS changes. Theme edits now reach pages that never load the Stillwind client runtime, such as resumable Qwik pages, without importing the CSS entrypoint or reloading the page.
