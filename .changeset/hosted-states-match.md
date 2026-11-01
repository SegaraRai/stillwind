---
"stillwind": patch
---

Variants that test a shadow host now match. `host:hover:`, `hover:` with `selector: ":host"`, and `@apply hover:flex` inside `:host` generate `:host()` forms that browsers accept. The new `unmatched-host-selector` rule reports variants such as `host:in-[.page]:`, which read elements outside the shadow tree and cannot match.
