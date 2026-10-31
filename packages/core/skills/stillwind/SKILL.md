---
name: stillwind
description: Write and fix styles in a project that uses Stillwind, by loading the relevant pages of the bundled documentation.
---

# Stillwind

Read only the pages that the task needs. Use the documentation of the installed
package in `node_modules/stillwind/docs/`, so that it matches the project's
Stillwind version. The `docs/` paths below are relative to the package root.
When this skill is installed without a local package, follow the skill's link to
its real directory and use `../../docs/` in the same package. If the skill was
copied without the package, start from https://stillwind.dev/docs/llms.txt,
which lists the Markdown version of each page.

## Task routing

| Task                                                   | Pages                                                               |
| ------------------------------------------------------ | ------------------------------------------------------------------- |
| Set up a project, choose an integration                | `docs/index.md`, `docs/integrations.md`                             |
| Migrate existing styles                                | `../migrate-to-stillwind/SKILL.md`                                  |
| Recommended practices and mistakes that produce no CSS | `docs/guides/recommended-practices.md`                              |
| Utilities, variants, conditions, runtime values        | `docs/guides/style.md`, `docs/macros/style.md`                      |
| Shared styles, states, and component options           | `docs/guides/reusable-styles.md`, `docs/macros/cva.md`              |
| Breakpoints and media queries in JavaScript            | `docs/guides/responsive-styles.md`, `docs/macros/media.md`          |
| Styles that depend on a parent, sibling, or anchor     | `docs/guides/element-relationships.md`, `docs/macros/ref/index.md`  |
| Animations                                             | `docs/guides/animations.md`, `docs/macros/keyframes.md`             |
| View transitions                                       | `docs/guides/view-transitions.md`, `docs/macros/view-transition.md` |
| Theme tokens and design-system rules                   | `docs/guides/design-systems.md`                                     |
| Custom utilities and variants                          | `docs/at-rules/index.md`, then the page of the at-rule              |
| Available utilities                                    | `docs/utilities.md`                                                 |
| `stillwind.config.ts`                                  | `docs/configuration.md`                                             |
| CLI, CI, and lint presets                              | `docs/guides/lint-adoption.md`, `docs/cli.md`                       |
| Editor setup and language server                       | `docs/editors.md`                                                   |
| Prettier and other formatters                          | `docs/formatters.md`                                                |
| Official plugins and Iconify icons                     | `docs/plugins/index.md`, `docs/plugins/iconify.md`                  |
| A lint finding                                         | `docs/lints/<rule>.md`                                              |
| A compiler error such as `SW01001`                     | `docs/errors/<code>.md`                                             |
| Shadow DOM and Web Components                          | `docs/guides/web-components.md`                                     |
| Which declaration wins when styles are composed        | `docs/concepts/css-ordering.md`                                     |
| What the compiler generates                            | `docs/concepts/how-styles-compile.md`                               |

## Working rules

- Follow `docs/guides/recommended-practices.md`. Keep styles in `style()` calls
  on their elements, and express states with attributes and variants.
- Use theme tokens instead of arbitrary values. When a value is missing, ask the
  user before you add a token.
- Write custom utilities with `@apply` and variants, not with hand-written
  selectors or media queries.
- After you change styles, run `stillwind lint --fix` and `stillwind fmt` through
  the project's package manager, such as `vp exec stillwind lint`. Add
  `--format json` when you parse the output.
- Read the page of each remaining finding before you fix it. Do not add a
  suppression comment or relax the `lint` setting unless the user agrees. A
  suppression comment names the rule and gives the reason after `--`.
- Check the result in the browser when a change can affect the layout.
