---
"stillwind": patch
---

Standard CSS keywords, function names, and at-rule names in arbitrary values and authored CSS now match ASCII case-insensitively, while custom `--` identifiers and function names remain case-sensitive. For example, values such as `INITIAL`, `AUTO`, `CALC(1PX + 2px)`, `VAR(--space)`, `LIGHT-DARK(red, blue)`, and `@PROPERTY` rules are now recognized as standard CSS syntax.
