---
"@stillwind/integration": patch
---

Build webpack and Rspack applications faster with `StillwindPlugin`. The producer loaders of a compiler now share one design system, compiler session, and type checker while the configuration and CSS files stay unchanged, and document CSS reconstruction reuses their compiled modules instead of compiling every producer again. In watch mode, the shared checker receives the files that webpack or Rspack reports as changed before each compilation.
