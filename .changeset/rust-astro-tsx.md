---
"stillwind": minor
---

Astro components are now converted to TSX for analysis with `@astrojs/astro2tsx`, the converter that the Astro language server uses, instead of `@astrojs/compiler`. For Astro support, install `@astrojs/astro2tsx` `^0.1.3`. Stillwind no longer uses `@astrojs/compiler`.

Analysis now finds frontmatter in the same places as the Astro 7 build. A `---` fence that follows template text, such as an HTML comment, is template text and does not start frontmatter.
