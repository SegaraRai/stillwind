---
"stillwind": patch
---

Arbitrary properties whose name is not a CSS property name, such as `[&:focus]`, `[:focus]`, and `[1:2]`, now report `invalid-utility` and emit no CSS. Add a utility after a variant, as in `[&:focus]:flex`, to style with a selector.
