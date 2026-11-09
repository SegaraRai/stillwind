---
"@stillwind/native": patch
---

Keep pseudo-elements outside group and peer selectors, so variants such as `before:group-hover:flex` match. Custom wrapper variants such as `:is(.theme>&)` now also compose with `group-*` and `peer-*` without spaces around the combinator.
