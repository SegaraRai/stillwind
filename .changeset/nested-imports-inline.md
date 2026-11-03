---
"stillwind": minor
---

An `@import` inside a style rule, or inside a `@layer`, `@supports`, `@media`, or other conditional group, now inlines the imported stylesheet in place, as Tailwind CSS v4 does. For example, `.widget { @import "stillwind/preflight.css"; }` applies the reset only to the descendants of `.widget`. Previously, the compiled CSS kept the raw `@import`, and browsers ignored it. An unresolved nested import is now reported as `invalid-css-import` at its source location.
