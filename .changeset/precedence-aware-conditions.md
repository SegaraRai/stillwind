---
"stillwind": patch
"@stillwind/language-server": patch
---

Hover previews now parenthesize conditions by JavaScript operator precedence, so a condition such as `/[)(]/.test(value)` or `a ?? b` is shown correctly. Type checking also accepts producer calls whose argument list ends with a comment, such as `style.ref(value // note,` followed by a new line.
