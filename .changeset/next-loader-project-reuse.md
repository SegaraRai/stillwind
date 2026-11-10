---
"@stillwind/integration": patch
---

Build and edit Next applications faster. The Stillwind loaders for Turbopack and webpack in Next now reuse the evaluated configuration, the design system, and the type checker across modules in one process while the configuration and CSS files stay unchanged. Document CSS is compiled again only for the modules whose sources or imported modules changed since the previous reconstruction in the same process.
