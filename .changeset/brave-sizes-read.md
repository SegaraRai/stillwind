---
"stillwind": patch
---

`conflicting-utilities` no longer reports a font size utility together with a `leading-*` utility, such as `text-sm leading-relaxed`. The font size utility reads the line height that the `leading-*` utility sets, so the result does not depend on their order.
