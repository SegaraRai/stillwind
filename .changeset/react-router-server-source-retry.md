---
"@stillwind/integration": patch
---

A React Router production build no longer fails at random with `Server-only module referenced by client` for a route that imports a `.server` module with `?stillwind.css`, or whose styles depend on a `.server` module. Rolldown could pass the option that Stillwind uses to skip React Router's check to a different resolution that runs at the same time. Stillwind now resolves such an import again when React Router reports this error. Imports that remain in client code are still checked.
