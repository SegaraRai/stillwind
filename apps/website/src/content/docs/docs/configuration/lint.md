---
title: Lint configuration
description: Choose presets, set rule severities and options, suppress findings, and configure sorting.
slug: docs/configuration/lint
---

The `lint` field of a project selects the rules that the CLI and the editor
report. [Checks in the editor and CI](/docs/guides/lint-adoption/) explains
where the rules run.

## Presets and severities

```ts
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

## How the settings combine

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

## Rule options

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

| Rule                         | Options                                                                                                                 |
| ---------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `conflicting-utilities`      | `allowShorthandOverrides`: allows the supported overrides between shorthand and longhand utilities; defaults to `false` |
| `invalid-marker-use`         | `ignoredClasses`: marker class names that the rule ignores                                                              |
| `no-arbitrary-spacing`       | `properties`: properties to check in addition to margin, padding, and gap                                               |
| `no-restricted-utilities`    | `utilities`: utility patterns, each with an optional message                                                            |
| `no-restricted-declarations` | `rules`: patterns for properties and values; every matching entry applies                                               |
| `no-restricted-length-units` | `rules`: property patterns and the units to `disallow`; the last matching entry applies                                 |
| `no-restricted-theme-tokens` | `tokens`: patterns for theme custom properties; required when the rule is on                                            |
| `prefer-canonical-utilities` | `pxToRem`: compares `px` values with `rem` values by using `rootFontSize`; defaults to `true`                           |
| `prefer-variant-directive`   | `pxToRem`: treats `px` and `rem` in media queries as equal at 16px per `rem`; defaults to `false`                       |

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
