---
title: Configuration
description: Set up projects in stillwind.config.ts, and look up every project field.
slug: docs/configuration
---

`stillwind.config.ts` describes your projects. The build integration, the CLI,
and the editor all read the same file.

```ts
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

| Field  | Value                                                                  |
| ------ | ---------------------------------------------------------------------- |
| `lint` | Presets and rules; see [Lint configuration](/docs/configuration/lint/) |
| `sort` | Where `stillwind fmt` sorts utilities, or `false` to disable sorting   |

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
