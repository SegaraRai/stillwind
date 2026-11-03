---
"stillwind": minor
---

An `@import` inside a style rule, or inside a `@layer`, `@supports`, `@media`, or other conditional group, now inlines the imported stylesheet in place, as Tailwind CSS v4 does. For example, `.widget { @import "stillwind/preflight.css"; }` applies the reset only to the descendants of `.widget`. Previously, the compiled CSS kept the raw `@import`, and browsers ignored it. Only plain CSS can be imported inside a style rule: when the imported file or its imports contain a definition, `@stillwind-slot`, or a `@layer` statement, such as `stillwind` itself, the import is left out. `invalid-css-import` reports that import and an unresolved nested import at their source locations.

A definition such as `@utility` or `@theme`, a Tailwind statement such as `@plugin`, or a `@layer` statement written inside a style rule is now ignored and reported by `invalid-css-directive`. Previously, a nested `@utility` was registered while a nested `@theme` was not, the raw at-rules stayed in the compiled CSS, and a nested `@layer` statement failed the build with a parse error.
