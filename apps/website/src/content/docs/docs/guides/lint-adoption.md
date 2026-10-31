---
title: Checks in the editor and CI
description: Catch misspelled utilities, conflicts, and project policy violations with the CLI, CI, and the editor.
slug: docs/guides/lint-adoption
---

You want to catch misspelled utilities, conflicting styles, and values outside
your theme before you merge a change. Stillwind checks them with lint rules.
The CLI, CI, the editor, and AI agents all use the same rules from
`stillwind.config.ts`.

## What the build checks

The build stops on compiler errors, such as a JavaScript syntax error or a
`style()` call that cannot compile. See [Compiler errors](/docs/errors).

The build does not run lint rules. A misspelled utility such as `bg-acent`
produces no CSS, and the page still renders without that style. Run the CLI or
use the editor to find such mistakes.

## Run the CLI

```sh
vp i -D @stillwind/cli
```

Run these commands from the application root:

```sh
vp exec stillwind lint
vp exec stillwind lint --fix
vp exec stillwind fmt
```

`lint` reports findings. `lint --fix` also applies the automatic fixes that the
rules provide. `fmt` sorts the utilities in `style()` calls. Review the changes
before you commit a large fix.

The CLI checks the files that `files` selects in `stillwind.config.ts`, including
files that the application does not import.

## Choose rules

Without a `lint` setting, the project uses the `recommended` preset. Add a
`lint` setting to add presets or change rules:

```ts title="stillwind.config.ts"
import { defineConfig, lintPresets } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "./src/stillwind.css",
      files: ["src/**/*.{ts,tsx,vue,svelte,astro}"],
      lint: {
        extends: [lintPresets.recommended, lintPresets.stylistic],
        rules: { "no-arbitrary-colors": "warn" },
      },
    },
  ],
});
```

When you write a `lint` setting, list `lintPresets.recommended` in `extends`.
A `lint` setting without `extends` uses no preset.

| Preset        | Contents                                                               |
| ------------- | ---------------------------------------------------------------------- |
| `recommended` | Invalid input, conflicting styles, and other likely mistakes           |
| `strict`      | Every `recommended` rule as an error, and unused suppression comments  |
| `stylistic`   | Preferred spellings and utility sorting; combine it with `recommended` |

Each rule has a severity. `warn` reports a warning, `deny` reports an error,
and `off` disables the rule and its fixes. To adopt a new rule, set it to `warn`
first, fix the existing code, and then change it to `deny`.

[Configuration](/docs/configuration#lint) describes rule options, and the
[lint rule reference](/docs/lints) lists every rule.

## Run the checks in CI

```sh
vp exec stillwind lint
vp exec stillwind fmt --check
```

`lint` fails when an error remains. `fmt --check` fails when utilities need
sorting. Warnings do not fail the command. Add `--max-warnings 0` to fail on
warnings too.

In GitHub Actions, the CLI prints findings as annotations that link to the reported lines.

## Let AI agents run the checks

Tell your agent to run `stillwind lint --fix` and `stillwind fmt` after it edits
styles. The agent sees the same findings as you. Add `--format json` when the
agent or a script reads the results.

## Check while you edit

The VS Code extension shows findings as you type, including in unsaved files. It also offers the
automatic fixes as Quick Fixes, and it can fix and sort on save. See [Editors](/docs/editors) for
the setup.

## Allow an exception

When code breaks a rule on purpose, add a comment on the line above it. Name the
rule, and give the reason after `--`:

```ts
import { style } from "stillwind/react";

// swlint-disable-next-line no-arbitrary-colors -- External brand color.
export const badge = style("bg-[#123456]");
```

A comment hides the finding only. It does not make a misspelled utility produce
CSS, and it does not hide compiler errors.
