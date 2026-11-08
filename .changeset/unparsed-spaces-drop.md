---
"@stillwind/native": patch
"@stillwind/integration": patch
---

`minifyCss()` now removes whitespace next to commas in values that Lightning CSS does not parse, such as custom property values and values that contain `var()`. Inside such a `calc()` or other math function, it also removes whitespace next to `*` and `/`. For example, `calc(var(--spacing) * 4)` becomes `calc(var(--spacing)*4)`, so minified CSS is smaller.
