---
"stillwind": patch
"@stillwind/cli": patch
---

The `stillwind` package now bundles `docs/llms.txt`, a single-file index of all bundled Markdown documentation pages with relative paths and descriptions, and updates `AGENTS_TEMPLATE.md` and `skills/stillwind/SKILL.md` so coding agents can look up design-system tokens, utilities, and generated CSS without broken cross-skill links. The public `style()` overload now appears before the internal checker overload so TypeScript diagnostics report the public signature first. `stillwind inspect` now prints `docs:` lines for findings in text output, matching `stillwind lint`.
