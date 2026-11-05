---
"stillwind": patch
"@stillwind/integration": patch
---

Astro components whose frontmatter ends with a `//` line comment now pass `stillwind lint` and the build. Previously, such components failed with SW31005 ("Astro frontmatter coordinates do not match source"), so a trailing comment such as `// TODO` or a tool directive could fail the whole site build.
