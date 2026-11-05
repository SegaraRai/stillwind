---
"stillwind": patch
"@stillwind/integration": patch
---

Rules that a Tailwind CSS JavaScript plugin nests inside `@scope` within a selector, such as `@scope (&)`, now stay inside the `@scope` block, as the same rules in a CSS `@utility` do. Previously, each nested selector moved into the `@scope` prelude, so a selector that used `:scope` matched the document root instead of the scoping root and the rule never applied. For example, daisyUI's `join` lost the radius of its outer corners.

A plugin that passes `addUtilities()` or `addComponents()` an `@scope` rule outside any selector, such as `{ "@scope (.theme)": { ".item": { … } } }`, now fails with an error. Previously, Stillwind generated CSS that styled other elements than Tailwind CSS does, without a diagnostic.
