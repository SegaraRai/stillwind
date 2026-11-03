---
"stillwind": patch
---

A `@variant` block in the body of a `@utility` now applies its variants, as it does in a style rule. Directly in the body, the variants apply to the element that uses the utility. Inside a nested rule, they apply to that rule's selector. Previously, the compiled CSS kept the raw `@variant` rule, and browsers ignored those styles. `prefer-variant-directive` now also reports conditions in `@utility` bodies that a variant can replace.
