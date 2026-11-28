---
"@stillwind/language-server": patch
---

Plugin catalog completions in stylesheets now find the `@apply` candidate or `@variant` sequence at the cursor from the CSS structure, so quotes or `;` inside an arbitrary value such as `content-['a;b']` no longer cut the candidate short, and text in comments and strings no longer triggers plugin lookups.
