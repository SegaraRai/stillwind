## Stillwind

- This project styles elements with [Stillwind](https://stillwind.dev/). Stillwind compiles Tailwind CSS v4 utilities in `style()` calls into element props and static CSS.
- Before you change styles, read the relevant Markdown docs in `node_modules/stillwind/docs/`. Start with `guides/recommended-practices.md`; `docs/llms.txt` indexes every page, and `node_modules/stillwind/skills/stillwind/SKILL.md` maps tasks to pages.
- After you change styles, run `stillwind check --fix` through the project's package manager, and fix the remaining findings.
- When you need to debug styles or look up an unfamiliar token or utility, use `stillwind inspect <file>:<line>`, `stillwind theme --source project`, or `stillwind utilities <name>` (add `--format json` when parsing the output).
