---
"stillwind": minor
---

`@utility` and `@stillwind-utility` accept the `(exclusive)` header descriptor. For a named value, the first `(exclusive)` definition of a name whose `--value()` accepts the value decides the theme namespace or other source, and other `(exclusive)` definitions apply only when they read the value from the same source.

Built-in utilities that read a value from several sources now resolve a named value as Tailwind CSS does. For example, when `@theme` defines both `--shadow-plain` and `--color-plain`, `shadow-plain` sets only the shadow, and `text-plain` sets only the color. This applies to `shadow-*`, `inset-shadow-*`, `drop-shadow-*`, `text-shadow-*`, `text-*`, `decoration-*`, `ring-*`, `inset-ring-*`, `outline-*`, and the `border-*` utilities. Previously, such a utility set the declarations of every matching namespace. When a color token has a number as its name, such as `--color-2`, `border-2`, `ring-2`, `inset-ring-2`, and `outline-2` now set only the color, and `decoration-2` sets only the thickness.

In the compiler's utility order, `outline-<number>`, `ring-<number>`, and `inset-ring-<number>` now follow the color utilities of the same root, and `decoration-<number>` now precedes them. `stillwind lint --fix` updates class lists that `prefer-sorted-utilities` reports.
