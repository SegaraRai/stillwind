---
"stillwind": patch
---

`ambiguous-style-base-order` and `interfering-imported-style-base` findings now include a note that reordering the `style()` arguments does not change which declaration wins. Fix these findings by building the overriding style on the other style or by removing the property from one of them.
