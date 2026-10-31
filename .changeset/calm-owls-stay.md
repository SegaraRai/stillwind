---
"stillwind": patch
---

`prefer-canonical-utilities` no longer replaces one theme token with another that only has the same value, such as `w-96` with `w-sm`. Fixed values now prefer spacing multiples, such as `w-112` for `w-[448px]`, and stay as written when several unrelated named tokens match.
