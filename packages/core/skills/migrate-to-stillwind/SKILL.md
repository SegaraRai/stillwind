---
name: migrate-to-stillwind
description: Migrate a project from Tailwind CSS, plain CSS, CSS Modules, preprocessors, or CSS-in-JS to Stillwind, after agreeing with the user how far to migrate.
---

# Migrate to Stillwind

Use the documentation in `node_modules/stillwind/docs/`. Without a local
package, use `../../docs/` relative to this skill's real directory, following
any link. If the skill was copied without the package, start from
https://stillwind.dev/docs/llms.txt, which lists the Markdown version of each page.

Read these pages before you change code:

- `docs/guides/migrating-from-tailwind.md` or `docs/guides/migrating-from-css.md`
- `docs/guides/tailwind-compatibility.md` for a Tailwind CSS project
- `docs/guides/recommended-practices.md`
- `docs/integrations.md` for the build tool

## Inspect the project

Check the working tree first, and keep unrelated changes. Then count what the
migration touches, so that the user can choose its scope:

- files and elements with class strings, and class helpers such as `clsx`,
  `classnames`, `tailwind-merge`, and `cva`;
- stylesheets, CSS Modules, preprocessor files, and CSS-in-JS calls;
- Tailwind configuration, `@config`, plugins, and import options;
- arbitrary values, such as `text-[#abcdef]` and `max-[768px]:`, and repeated
  literal values in CSS;
- `group-*` and `peer-*` variants, parent and sibling selectors, global
  `@keyframes`, and classes that code toggles.

## Agree on the scope

Explain these levels to the user. For each level, give the counts that it
affects, one before-and-after example from the project, and whether the pages
can look different. Recommend one level, and wait for the user's choice.

| Level | Tailwind CSS projects                                                                                                                                                                            | CSS projects                                                              | Look       |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------- | ---------- |
| 1     | Move classes into `style()`, replace class helpers, move the configuration                                                                                                                       | Move rules into `style()`; values without a token become arbitrary values | Unchanged  |
| 2     | Level 1, and replace arbitrary values with theme tokens and enable the design-system rules                                                                                                       | Level 1, and collect repeated values into theme tokens                    | Can change |
| 3     | Level 2, and adopt Stillwind idioms: refs instead of `group-*` and `peer-*`, keyframes next to their components, attributes instead of toggled classes, custom utilities instead of separate CSS | Same as Tailwind CSS projects                                             | Can change |

The user can also limit the migration to some directories or components. When
you cannot ask the user, migrate to level 1 and report the higher levels as
next steps.

## Migrate

1. Set up Stillwind as in steps 1 to 6 of `../setup-stillwind/SKILL.md`. In a
   CSS project, leave out the preflight reset until the old CSS is gone, as
   `docs/guides/migrating-from-css.md` describes.
2. Migrate in batches, such as one directory or one group of components. After
   each batch, build the project and run `stillwind lint`.
3. For many mechanical changes, you can write a one-off codemod for this
   project. Keep it outside the source directories, review its diff, and delete
   it afterward.
4. Remove an old dependency or stylesheet only when nothing uses it. The
   `tailwind` field of `stillwind.config.ts` still requires the `tailwindcss`
   package.

Do not change the look of the pages beyond the chosen level without asking.
Do not add suppression comments or relax the `lint` setting to pass the checks
unless the user agrees.

## Verify

- Build the project, and run `stillwind lint` and `stillwind fmt`.
- Compare the migrated pages with the old version in a browser when you can.
  Otherwise, list the pages that the user should check.

## Finish

- Offer to add the contents of `node_modules/stillwind/AGENTS_TEMPLATE.md` to
  the project's instruction file, such as `AGENTS.md` or `CLAUDE.md`. Ask
  before you create or change the file, and keep its other content.
- Report the chosen level, the changes, the verification results, and what
  remains: unresolved findings, plugins that did not work, code that you left
  unchanged, and the work for the next level. Do not commit unless the user asks.
