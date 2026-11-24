---
"@stillwind/integration": patch
---

Vite development servers now load fonts, images, and other files that the stylesheet references from a directory whose name contains `@`, `+`, or `:`, such as a scoped package like `@fontsource-variable/geist`. Before this fix, the development CSS referred to these files by URLs that Vite did not serve, so the page received an HTML response instead of the file. Production builds were not affected.
