---
"@stillwind/integration": patch
---

Apply edits faster in Next development servers that use Turbopack. The Stillwind loaders of a worker process now keep the project files in one type checker between compilations and watch the application directory for changes, so an edit no longer reloads the whole project into the checker.
