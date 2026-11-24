---
"stillwind": patch
---

The generated CSS now declares a value from a `@theme inline` block when the CSS refers to it with `var()`, as Tailwind CSS does. For example, `rounded-[min(var(--radius-md),12px)]` with `--radius-md` in `@theme inline` now gets `--radius-md` in the theme layer, instead of an undefined variable and a border radius of `0`. Values from a `@theme reference` block are never declared.
