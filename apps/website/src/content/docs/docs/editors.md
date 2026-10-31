---
title: Editors
slug: docs/editors
description: Install the Stillwind extension, sort and fix on save, and connect other editors to the language server.
---

The Stillwind language server shows diagnostics, generated CSS, and completion
from the active design system while you edit. It reads the same
`stillwind.config.ts` as the CLI and the build, and it checks unsaved files too.
Create that file with [Getting started](/docs) first.

## VS Code

Install **Stillwind** from the
[Visual Studio Marketplace](https://marketplace.visualstudio.com/items?itemName=stillwind.stillwind)
or [Open VSX](https://open-vsx.org/extension/stillwind/stillwind). An editor that
installs extensions from Open VSX uses the same package.

The extension needs a trusted workspace, because it runs your project
configuration as JavaScript.

### Features

- Hover over utilities, `style()` calls, or bound variables to see the generated CSS.
- Complete utilities, variants, and theme values.
- Complete quoted `@plugin` paths, and follow links to their modules.
- See colors, and open color pickers in utilities and theme references.
- Apply compiler Quick Fixes.
- Run **Stillwind: Sort Utilities**.

Stylesheets have completion for Stillwind at-rules, and VS Code's CSS service does
not report these at-rules as unknown.

### Language server version

Desktop VS Code runs the `@stillwind/cli` version that the workspace installs, so
editor diagnostics match the compiler of that version. Install
[the CLI](/docs/cli) in the project to get this match.

The extension uses its bundled language server when the workspace has no CLI, or
when the CLI fails to start. The **Stillwind** output channel shows which server
is running.

### Settings

| Setting                                   | Value                                                                                          |
| ----------------------------------------- | ---------------------------------------------------------------------------------------------- |
| `stillwind.configFile`                    | A configuration path relative to the workspace root; empty searches for `stillwind.config.*`   |
| `stillwind.completion.registryDirectives` | `true` suggests [registry directives](/docs/at-rules/registry-directives) in utility libraries |

Run **Stillwind: Restart Language Server** to reload configuration and dependencies.

The `preview` field of each project in `stillwind.config.ts` controls the comments
after values in CSS previews:

| Option             | Comment                                                      |
| ------------------ | ------------------------------------------------------------ |
| `themeValues`      | Resolved theme values and evaluated `calc()` results         |
| `pixelEquivalents` | Pixel values for `rem`                                       |
| `colorEquivalents` | Hexadecimal sRGB colors; `"exact"` omits colors outside sRGB |

All three are enabled by default. With the default `colorEquivalents` value,
`"approximate"`, a `≈` marks colors that the preview maps into sRGB. `false` turns
off an option.

### Sort and fix on save

Use a [document formatter](/docs/formatters) for layout and whitespace, and add
Stillwind's Code Actions:

```json
{
  "editor.formatOnSave": true,
  "editor.codeActionsOnSave": {
    "source.fixAll.stillwind": "explicit",
    "source.sort.stillwind": "explicit"
  }
}
```

`source.fixAll.stillwind` applies the enabled lint fixes and respects suppression
comments. `source.sort.stillwind` sorts whatever the lint severity is.

`lint: false` in the project disables lint rules and fixes. `sort: false` also
disables explicit sorting commands. The
[`sort` field](/docs/configuration#sorting) selects what the editor sorts.

### Web VS Code

The Web extension always uses the bundled language server, and it needs a host with
cross-origin isolation. These limits apply:

- Project configuration can import browser-compatible modules. Node built-ins are
  not available.
- The workspace filesystem must be able to read the dependencies.
- The default browser provider supports JS, JSX, TS, and TSX. Vue, Svelte, and
  Astro files need a custom source or projection provider.

## Other editors

An editor with a Language Server Protocol client can start the language server from
the CLI. Install [the CLI](/docs/cli) in the project, and run this command in the
workspace:

```sh
stillwind lsp --stdio
```

The server discovers the configuration from each workspace folder.
