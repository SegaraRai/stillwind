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

Use this structure:

```md
<One or two sentences that summarize the release for users.>

## Breaking changes

- <Change, followed by the migration step>

## Features

- <New capability>

## Fixes

- <Corrected behavior>

## Changelogs

<The existing list of links>
```
