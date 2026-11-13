---
"stillwind": patch
---

Design-system queries and editor previews now show utilities whose CSS has an empty declaration after a nested `@scope` block, such as daisyUI `join`. Before this fix, `stillwind utilities join` and a `*` utility listing for a daisyUI project failed with `CSS parse failed: Unexpected end of input`.
