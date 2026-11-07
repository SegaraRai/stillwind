---
"stillwind": patch
---

`conflicting-utilities` no longer reports a side utility together with a style utility that sets its style, such as `border-t border-dashed` or `divide-y divide-dashed`. The result of these pairs does not depend on their order.
