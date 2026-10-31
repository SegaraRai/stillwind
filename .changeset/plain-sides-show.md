---
"stillwind": patch
---

`prefer-logical-properties` now reports physical declarations that it previously missed: shorthands for some sides with any value, such as `[border-top:var(--border)]` and `[border-left-radius:1px]`, and shorthands with different values for each side, such as `[padding:var(--a)_1rem]` and `[size:10px_20px]`.
