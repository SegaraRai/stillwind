---
"@stillwind/integration": patch
---

Vite development servers now reload CSS for every file that is added to a directory that a CSS plugin lists, such as an icon directory. Before this fix, a file that was created while the server started or while the design system reloaded could be missed until the next change in that directory.
