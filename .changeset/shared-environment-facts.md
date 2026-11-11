---
"@stillwind/host": minor
"@stillwind/integration": patch
---

Vite builds and development servers now run TypeScript type collection once for all environments, such as client and SSR, while the environments resolve imports of source files to the same files. This shortens builds and the first page load. Packages may resolve differently, for example when SSR externalizes them. An environment that resolves an import of a source file differently continues with its own type collection.

Hosts that share a `CompilationProject` can set `resolverDomain` on `CompilerHost` to name their resolver. When another domain resolves an import of a source file differently, preparation and compilation reject with `ResolutionMismatchError`. Prepared import links no longer point the checker at package runtime files, so the checker resolves package types itself.
