---
"@stillwind/integration": patch
---

Vite-based integrations, including Astro, now warn when application code or a framework option such as Starlight's `customCss` imports the CSS entrypoint selected in `stillwind.config.ts`. The integration already delivers the compiled CSS, and importing the entrypoint also sends its uncompiled `@theme` and `@utility` rules to the page. Remove the import to fix the warning.
