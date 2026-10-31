---
"stillwind": patch
---

`prefer-logical-properties` no longer reports a shorthand that gives both axes the same value, such as `overflow-hidden` or `truncate`, and it no longer reports `sr-only` or `not-sr-only`, whose declarations you cannot change. `no-restricted-length-units` also skips `sr-only` by default.
