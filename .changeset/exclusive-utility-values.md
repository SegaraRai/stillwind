---
"stillwind": minor
---

`@utility` and `@stillwind-utility` accept the `(exclusive)` header descriptor. For a named value, the first `(exclusive)` definition of a name whose `--value()` accepts the value decides the theme namespace or other source, and other `(exclusive)` definitions apply only when they read the value from the same source.

Built-in utilities that read a value from several sources now resolve a named value as Tailwind CSS does. For example, when `@theme` defines both `--shadow-plain` and `--color-plain`, `shadow-plain` sets only the shadow, and `text-plain` sets only the color. This applies to `shadow-*`, `inset-shadow-*`, `drop-shadow-*`, `text-shadow-*`, `text-*`, and the `border-*` utilities. Previously, such a utility set the declarations of every matching namespace. When a color token has a number as its name, such as `--color-2`, `border-2` now sets only the border color.
