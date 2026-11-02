Instructions for a coding agent: write the GitHub Release notes for Stillwind {{tag}}.

{{range}}

Changelog sections:

{{changelogs}}

The links point to commit {{commit}}. Read every linked section before you write.

Follow these rules:

- Write in English for people who use Stillwind. Follow the Writing Guidelines in `AGENTS.md`
  at the root of the repository.
- Use only facts from the changelog sections. Do not add internal refactoring, tests, or CI
  changes.
- Read the code, and run examples or tests in the checkout when they help you confirm an
  entry. Restore every file that you change for this check.
- When the code or a check disagrees with a changelog entry, do not correct the entry from your
  own conclusion. Describe the difference in your report to the maintainer.
- Merge entries that describe the same change in several versions or packages.
- Leave out the "Updated dependencies" lists.
- A minor version before 1.0 can break existing code. List an entry under "Breaking changes"
  when it tells users to change configuration, rule names, or code. Keep its migration step.
- Omit a heading that has no entries.
- Keep the "Changelogs" section and its links unchanged at the end.
- The notes replace the whole body, including the request above this comment and the comment
  itself.

Start each entry with a scope label that names the part of Stillwind where users see the
change. Choose the scope from the change itself, not from the package whose changelog lists it.
For example, `@stillwind/cli` bundles the language server, so its changelog can list a VS Code
fix. Use these labels, in this order, when they fit:

1. `Core`: generated CSS, utilities, variants, theme tokens, configuration, diagnostic rules, and
   build integrations. Use `Core` when the change is the same in every tool, such as a rule that
   both `stillwind lint` and the editor report.
2. `CLI`: `stillwind` commands, their options, and their output formats.
3. `Editor`: the VS Code extension and the language server in other editors.
4. `Prettier`: `prettier-plugin-stillwind`.
5. `Iconify Plugin`: `@stillwind/plugin-iconify`. Label another plugin in the same way, such as
   `<Name> Plugin`, after `Iconify Plugin`.

When no label above fits a change, use a short label that names the part users see, and place
it after the labels above.

When an entry changes several scopes in different ways, join their labels in the order above,
such as `**CLI, Editor:**`, and write the entry once. Within each heading, order entries by
their first label in the order above.

Use this structure:

```md
<One or two sentences that summarize the release for users.>

## Breaking changes

- **<Scope>:** <Change, followed by the migration step>

## Features

- **<Scope>:** <New capability>

## Fixes

- **<Scope>:** <Corrected behavior>

## Changelogs

<The existing list of links>
```
