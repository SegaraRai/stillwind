---
title: Configuration
description: Set up projects in stillwind.config.ts, choose lint rules, and look up every project field.
slug: docs/configuration
---

`stillwind.config.ts` describes your projects. The build integration, the CLI,
and the editor all read the same file.

```ts title="stillwind.config.ts"
import { defineConfig } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "./src/stillwind.css",
      files: ["src/**/*.{ts,tsx,vue,svelte,astro}"],
    },
  ],
});
```

Each entry in `projects` selects a stylesheet and the files that belong to it.
Most applications need one project. A monorepo can list one project for each
application.

## Where Stillwind finds the file

The build integration searches from the application root upward. The CLI and
the editor search from the working directory or the workspace upward. The file
in the nearest directory wins.

Stillwind accepts `stillwind.config.ts`, `.mts`, `.cts`, `.js`, `.mjs`, `.cjs`,
and `.json`. In one directory, it uses the first of these that exists.

`defineConfig()` also accepts a function or an async function that returns the
settings. Stillwind calls it once and uses the result.

## Project fields

### Required

| Field   | Value                                                                              |
| ------- | ---------------------------------------------------------------------------------- |
| `css`   | Path to the stylesheet that defines the theme, utilities, and variants             |
| `files` | Globs for the files that the CLI and the editor check; the build does not use them |

### Scope

| Field      | Value                                                                                |
| ---------- | ------------------------------------------------------------------------------------ |
| `name`     | A unique name that selects this project from an integration or the CLI               |
| `root`     | The project directory, relative to the configuration file; defaults to its directory |
| `exclude`  | Globs that remove files from `files`; `node_modules` and `.git` are always excluded  |
| `tsconfig` | Path to the TypeScript configuration; defaults to `tsconfig.json` in the root        |

Paths in `css`, `files`, `exclude`, and `tsconfig` are relative to `root`.

### Checks and sorting

| Field  | Value                                                                                      |
| ------ | ------------------------------------------------------------------------------------------ |
| `lint` | Presets and rules; see [Lint](#lint)                                                       |
| `sort` | Where `stillwind fmt` sorts utilities, or `false` to disable them; see [Sorting](#sorting) |

### Generated CSS

| Field          | Value                                                                                                                        |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `target`       | Minimum browser versions, such as `["chrome111", "safari17"]`; removes fallbacks they do not need                            |
| `utilityOrder` | `"authored"` orders utilities as your stylesheet defines them; `"tailwind"` orders them as Tailwind does                     |
| `important`    | `true` makes every utility `!important`; a selector nests utilities under that selector                                      |
| `tailwind`     | A Tailwind configuration object; see [Migrating from Tailwind CSS](/docs/guides/migrating-from-tailwind/#move-configuration) |
| `plugins`      | Plugin objects for the `@plugin` rules of your stylesheet                                                                    |
| `rootFontSize` | Pixels per `rem`, used by lint fixes and editor previews; defaults to 16                                                     |
| `useCssIf`     | `true` lets runtime CSS conditions use the native CSS `if()` function; defaults to `false`                                   |

### Generated names

| Field            | Value                                                                               |
| ---------------- | ----------------------------------------------------------------------------------- |
| `classNamespace` | Text added to generated class names; use the same value in client and server builds |
| `hashLength`     | Number of hash characters in generated names, from 1 to 50; defaults to 5           |
| `prefix`         | Prefix for Stillwind's custom properties; defaults to `--sw-`                       |
| `tailwindPrefix` | Prefix for Tailwind-compatible custom properties; defaults to `--tw-`               |
| `naming`         | Functions that choose generated names yourself                                      |
| `namingRevision` | A string that you change whenever you change the `naming` functions                 |

### Editor

| Field     | Value                                                                                       |
| --------- | ------------------------------------------------------------------------------------------- |
| `preview` | Which comments CSS previews show: `themeValues`, `pixelEquivalents`, and `colorEquivalents` |

The [shared configuration API](https://github.com/SegaraRai/stillwind/blob/main/packages/config/src/compiler.ts)
describes the remaining fields for custom integrations, such as `sources`,
`projection`, `typeFacts`, and `automatic`.

## Lint

The `lint` field of a project selects the rules that the CLI and the editor
report. [Checks in the editor and CI](/docs/guides/lint-adoption/) explains
where the rules run.

### Presets and severities

```ts title="stillwind.config.ts"
import { defineConfig, lintPresets } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "./src/stillwind.css",
      files: ["src/**/*.{ts,tsx,vue,svelte,astro}"],
      lint: {
        extends: [lintPresets.recommended, lintPresets.stylistic],
        rules: {
          "unknown-utility": "deny",
          "no-arbitrary-colors": "warn",
        },
      },
    },
  ],
});
```

| Preset        | Contents                                                               |
| ------------- | ---------------------------------------------------------------------- |
| `recommended` | Invalid input, conflicting styles, and other likely mistakes           |
| `strict`      | Every `recommended` rule as an error, and unused suppression comments  |
| `stylistic`   | Preferred spellings and utility sorting; combine it with `recommended` |

The [lint rule reference](/docs/lints/) shows the severity of each rule in
`recommended`. Each rule page shows its severity in every preset.

Each rule has one of three severities:

| Severity | Effect                                                    |
| -------- | --------------------------------------------------------- |
| `warn`   | Reports a warning, which does not fail the CLI by default |
| `deny`   | Reports an error, which fails the CLI                     |
| `off`    | Reports nothing and applies no fixes                      |

### How the settings combine

| `lint` value                | Result                               |
| --------------------------- | ------------------------------------ |
| Not set                     | The `recommended` preset             |
| An object without `extends` | No preset; only the rules in `rules` |
| `false` or `{}`             | No rules and no fixes                |

Stillwind applies the presets in `extends` in order, and then `rules`. A later
setting replaces the severity of an earlier one. Options in objects merge, and
arrays replace earlier arrays. A severity alone, such as `"deny"`, keeps the
rule's options from the presets.

An unknown rule name is a configuration error. Turning off rules does not turn
off [compiler errors](/docs/errors/).

### Rule options

Some rules accept options. Write the rule as an object with `severity` and the
options:

```ts
const rules = {
  "no-restricted-utilities": {
    severity: "deny",
    utilities: [{ pattern: "absolute", message: "Use the positioned component." }],
  },
  "no-restricted-declarations": {
    severity: "deny",
    rules: [{ properties: ["position"], values: ["absolute"] }],
  },
  "no-restricted-length-units": {
    severity: "warn",
    rules: [{ properties: ["width", "height"], disallow: ["px"] }],
  },
  "no-restricted-theme-tokens": {
    severity: "deny",
    tokens: ["--color-blue-*"],
  },
};
```

| Rule                         | Options                                                                                                                                                                                                 |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `conflicting-utilities`      | `allowShorthandOverrides`: allows the supported overrides between shorthand and longhand utilities; defaults to `false`                                                                                 |
| `invalid-marker-use`         | `ignoredClasses`: marker class names that the rule ignores                                                                                                                                              |
| `no-arbitrary-spacing`       | `properties`: properties to check in addition to margin, padding, and gap                                                                                                                               |
| `no-restricted-utilities`    | `utilities`: utility patterns, each with an optional message                                                                                                                                            |
| `no-restricted-declarations` | `rules`: patterns for properties and values; every matching entry applies                                                                                                                               |
| `no-restricted-length-units` | `rules`: property patterns and the units to `disallow`; the last matching entry applies                                                                                                                 |
| `no-restricted-theme-tokens` | `tokens`: patterns for theme custom properties; required when the rule is on                                                                                                                            |
| `prefer-canonical-utilities` | `pxToRem` and `collapse` default to `true`; `logicalToPhysical` defaults to `false`; `preserveSpecificity` defaults to `true`. See the [rule options](/docs/lints/prefer-canonical-utilities/#options). |
| `prefer-variant-directive`   | `pxToRem`: treats `px` and `rem` in media queries as equal at 16px per `rem`; defaults to `false`                                                                                                       |

Patterns accept `*` for any text and `?` for one character. Utility and theme
token patterns are case-sensitive. Property and value patterns are not.

## Suppression comments

To allow one exception, add a comment that names the rule. Give the reason after
`--`:

```ts
import { style } from "stillwind/react";

// swlint-disable-next-line no-arbitrary-colors -- External brand color.
export const badge = style("bg-[#123456]");
```

| Comment                                          | Applies to                         |
| ------------------------------------------------ | ---------------------------------- |
| `swlint-disable-next-line <rule>`                | The next line                      |
| `swlint-disable-line <rule>`                     | The same line                      |
| `swlint-disable <rule>` … `swlint-enable <rule>` | The lines between the two comments |

In stylesheets, write the same text in `/* ... */` comments.

A comment hides the finding only. A misspelled utility still produces no CSS.
[invalid-lint-suppression](/docs/lints/invalid-lint-suppression/) reports a
malformed comment, and
[unused-lint-suppression](/docs/lints/unused-lint-suppression/) reports a
comment that suppresses nothing.

## Sorting

The `sort` field controls where `stillwind fmt` and the editor sort utilities:

| Option            | Sorts                                              | Default |
| ----------------- | -------------------------------------------------- | ------- |
| `stringLiterals`  | Utilities inside each string                       | `true`  |
| `arrays`          | Arguments, array items, and object entries by kind | `true`  |
| `applyUtilities`  | Utilities inside each `@apply` rule                | `true`  |
| `applyStatements` | Adjacent `@apply` rules                            | `false` |
| `variantBlocks`   | Adjacent `@variant` blocks                         | `false` |

`sort: false` disables sorting, including `stillwind fmt` and the
[prefer-sorted-utilities](/docs/lints/prefer-sorted-utilities/) rule. Turning off
only that rule keeps `stillwind fmt` working.

The CLI checks the files that `files` selects. File patterns on the command line
narrow that set; they cannot add files outside it.

## Several projects

When the file has several projects, each integration selects one:

1. If the integration has a `project` option, it selects the project with that `name`.
2. Otherwise, it selects the project whose `root` is the deepest directory that
   contains the application root.

If no project or more than one project matches, the build fails with a
configuration error. The CLI and the editor check every project.

## Override settings in the integration

The integration accepts options that change how it reads the file:

```ts
import { stillwind } from "@stillwind/integration/vite";

stillwind({ configFile: "config/stillwind.config.ts", project: "web" });
```

`configFile` selects another file, relative to the application root. Other
options, such as `css` or `lint`, replace the values of the selected project.
Objects merge with the file's values, and arrays replace them.
