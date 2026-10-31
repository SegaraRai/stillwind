---
"stillwind": minor
---

The `recommended` preset now reports `unknown-utility` and `unknown-variant` as errors, so `stillwind lint` fails on a misspelled utility or variant. Set a rule to `warn` in `lint.rules` to keep the previous severity.

`prefer-explicit-nesting-selector` is now `implicit-nesting-selector`, and its category changes from `style` to `hazard`. Update the rule name in `lint.rules` and in `swlint-*` comments.
