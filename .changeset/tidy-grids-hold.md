---
"stillwind": patch
---

`prefer-canonical-utilities` no longer rewrites an arbitrary value that starts and ends with separate function calls, such as `grid-cols-[var(--sidebar)_minmax(0,1fr)]`, into broken classes. `stillwind lint --fix` now keeps these values as written.
