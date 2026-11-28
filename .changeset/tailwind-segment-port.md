---
"stillwind": patch
---

Tailwind CSS plugins now split selectors, candidate modifiers, and arbitrary values, and reject invalid arbitrary values, by the same rules as Tailwind CSS. For example, a `,` inside `{}` no longer splits a value, and an arbitrary value with an unbalanced `)` or `]` is rejected. A theme value that refers to its own variable through `--name(...)` or a nested `var()` fallback is now also kept out of the generated variable declarations.
