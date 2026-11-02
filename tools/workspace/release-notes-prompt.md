Instructions for a coding agent: write the GitHub Release notes for Stillwind {{tag}}, check
that the release is ready to publish, and report the result to the maintainer.

## Release state

The Release workflow ran on commit {{commit}} on `main`. It has completed these steps:

1. It built and tested every package.
2. It uploaded the npm packages to the npm staging area. npm serves a staged version only after
   a maintainer approves it with 2FA, and the maintainer might not have approved it yet.
3. It published the VS Code extension to Visual Studio Marketplace and Open VSX.
4. It created this draft with the extension packages and `SHA256SUMS`.

The tag {{tag}} does not exist yet. GitHub creates it on commit {{commit}} when the maintainer
publishes the draft. A missing tag and a draft URL that starts with `untagged` are expected.

After the npm approval, the maintainer advances the `stable` branch to the release commit. This
push deploys the website and the playground. The maintainer might do it before or after the
publication of the draft.

## Tasks

1. Write the release notes as described in "Release notes", and replace the body of the draft.
2. Check the release state as described in "Release checks".
3. Report to the maintainer as described in "Report". Do not publish the draft until the
   maintainer tells you to.

## Release notes

{{range}}

Changelog sections:

{{changelogs}}

The links point to commit {{commit}}. Read every linked section before you write.

Follow these rules:

- Write in English for people who use Stillwind. Follow the Writing Guidelines in `AGENTS.md`
  at the root of the repository.
- Use only facts from the changelog sections. Do not add internal refactoring, tests, or CI
  changes.
- Read the code at commit {{commit}}, and run examples or tests in the checkout when they help
  you confirm an entry. When the checkout does not contain the commit, fetch it from `origin`.
  Do not switch branches, and restore every file that you change for this check.
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

## Release checks

Check these conditions:

- npm serves version {{version}} of every package under `packages/` and `plugins/` whose
  `package.json` is not private. Use `vp exec pnpm view <name>@{{version}} version`, because
  npm commands fail in this repository.
- Visual Studio Marketplace and Open VSX serve version {{version}} of the `stillwind.stillwind`
  extension for each target that the draft attaches a VSIX for.
- The `stable` branch on `origin` points to commit {{commit}}.

## Report

Report these items to the maintainer:

- The differences between the code and the changelog entries, if any.
- The result of each release check.
- The next action:
  - When npm does not serve a package, or a marketplace does not serve the extension, name the
    missing items and stop. For packages that npm does not serve yet, ask the maintainer to
    approve the staged versions with `vp exec pnpm stage approve`.
  - Otherwise, propose that the maintainer let you publish the draft. When `stable` does not
    point to the release commit, add a reminder to advance it. This step does not block the
    publication.

Do not report an expected state, such as the missing tag, as a problem.
