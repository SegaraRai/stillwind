---
"stillwind": patch
---

A module can now re-export its CSS value from `stillwind/module-css`, for example with `export { css as toolbarCss } from "stillwind/module-css"` or `export * from "stillwind/module-css"`, and can load it with `import("stillwind/module-css")`. The compiler replaces the requested module with the module's compiled CSS, in the same way as an `import`. Before this fix, these forms loaded the source placeholder, which throws at runtime. Type-only re-exports and `typeof import()` types stay unchanged.
