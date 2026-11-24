---
"stillwind": patch
---

Relationship variants such as `group-*` now apply each selector of a custom variant that lists several selectors. For example, with shadcn/ui's `@custom-variant data-disabled` that lists two `:where()` selectors, `group-data-disabled/checkbox:opacity-50` styles the descendants of `.group/checkbox` in both cases, and `repeated-nesting-selector` no longer reports it.
