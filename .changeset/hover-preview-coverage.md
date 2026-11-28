---
"@stillwind/cli": patch
"@stillwind/language-server": patch
---

Hover previews now annotate `color-mix()` and `light-dark()` color equivalents, evaluate `calc()` with `rem` dimensions when `themeValues` is disabled and inside `@media` / `@container` preludes, display registered `@property` definitions on `@stillwind-require-properties` arguments, preview `style.when()`, `style.ref()`, and `style.recipe()` calls and their variable references, and display the resolved JavaScript string literal for `style.media()` calls and references.
