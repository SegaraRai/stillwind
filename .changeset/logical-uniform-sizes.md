---
"stillwind": patch
---

`prefer-logical-properties` no longer reports a rule that sets every physical property of a group to one value, such as `size-6`, which sets `width` and `height` to the same value. The result does not depend on the writing mode, as for `p-4`. A utility that sets `width` and `height` in different rules is still reported.
