---
"stillwind": patch
---

`shadow-*`, `inset-shadow-*`, `drop-shadow-*`, and `text-shadow-*` utilities now use a theme value that is one `var()` reference as the shadow. For example, with `--shadow-surface: var(--surface-shadow)` in `@theme`, `shadow-surface` sets `--tw-shadow: var(--surface-shadow)`. Previously, when the theme also had `--color-surface`, the utility set only the shadow color, and the element had no shadow.
