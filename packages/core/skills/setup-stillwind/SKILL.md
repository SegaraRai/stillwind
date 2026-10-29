---
name: setup-stillwind
description: Set up Stillwind in a project that has no styles to migrate, such as a new project, using the bundled documentation.
---

# Set up Stillwind

Use the documentation in `node_modules/stillwind/docs/`. Without a local
package, use `../../docs/` relative to this skill's real directory, following
any link. If the skill was copied without the package, start from
https://stillwind.dev/docs/llms.txt, which lists the Markdown version of each page.

If the project already styles its pages with Tailwind CSS or other CSS, follow
`../migrate-to-stillwind/SKILL.md` instead.

## Steps

1. Find the package manager, the build tool, and the frameworks. Stillwind
   requires a build tool that `docs/integrations.md` lists. If the project has
   no build tool, tell the user and stop.
2. Read `docs/index.md` and the section of `docs/integrations.md` for the build
   tool.
3. Install `stillwind` as a dependency, and `@stillwind/integration` and
   `@stillwind/cli` as development dependencies. When the project uses
   Prettier, also install `prettier-plugin-stillwind` and add it to the Prettier
   configuration.
4. Create the CSS entry and `stillwind.config.ts`. Make `files` match the file
   extensions of the frameworks that the project uses.
5. Register the integration as the integration page describes. Keep the plugin
   order that it requires, such as Stillwind before the JSX transform.
6. Add `stillwind lint` and `stillwind fmt --check` to the project's check
   scripts or CI when the project has them.
7. A starter template often includes demo styles, such as `App.css`. Ask the
   user whether to rewrite the demo components with `style()` or to remove the
   demo.

## Verify

- Build the project, and start the development server if you can. Confirm that
  an element styled with `style()` receives its CSS.
- Run `stillwind lint`, and fix the findings.

## Finish

- Offer to add the contents of `node_modules/stillwind/AGENTS_TEMPLATE.md` to
  the project's instruction file, such as `AGENTS.md` or `CLAUDE.md`. Ask
  before you create or change the file, and keep its other content.
- Mention that `docs/agents.md` describes how to install the skills, and that
  the VS Code extension shows findings in the editor.
- Report the installed packages, the created and changed files, and the
  verification results. Do not commit unless the user asks.
