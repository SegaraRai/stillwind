---
"@stillwind/cli": patch
"@stillwind/language-server": patch
---

Hover previews in the language server now omit redundant empty `& {}` rules inside selector variants, trim trailing blank lines from CSS code blocks, annotate active theme declarations hovered in stylesheets, omit redundant hexadecimal comments on shorthand hex theme tokens, omit marker-class notes under enclosing variants, and avoid treating unrelated `.at()` and `.in()` calls as ref variants.
