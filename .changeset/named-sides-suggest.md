---
"stillwind": minor
---

When a physical utility conflicts with a logical utility, the `conflicting-utilities` finding shows how to use logical properties for both: it names a logical utility from your design system that writes the same value, such as `ms-4` for `ml-4` in `mx-auto ml-4`. When `writingModes` includes a vertical writing mode, as in the `stylistic` preset, it also names `pbs-2` for `pt-2` in `px-2 pt-2`. The suggestion uses the sides of horizontal left-to-right writing modes, appears only when `writingModes` includes `horizontal-tb`, and does not recommend logical properties over physical ones.
