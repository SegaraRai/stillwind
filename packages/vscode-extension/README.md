# Stillwind for Visual Studio Code

Stillwind compiles Tailwind CSS v4 utilities into component props and static CSS.
It uses theme rules and compiler checks to keep styling code in order, even when
AI agents write much of it.

`@stillwind/vscode-extension` provides Stillwind language support for VS Code.
It adds compiler diagnostics, Quick Fixes, utility completion, CSS previews,
color pickers, and sorting.

## Get started

Install the Desktop or Web VSIX with VS Code's **Extensions: Install from VSIX**
command. Desktop packages target a specific operating system and architecture.
The extension includes its language server; the CLI is not required.

Add a [Stillwind project configuration](https://github.com/SegaraRai/stillwind/blob/main/packages/core/README.md#configure-a-project)
to the workspace. The extension discovers `stillwind.config.*`; set
`stillwind.configFile` to select a path relative to the workspace root.
Use **Stillwind: Restart Language Server** to reload configuration and dependencies.

## Features

- Hover over utilities, `style()` calls, or bound variables to inspect generated CSS.
- Complete utilities, variants, and theme values from the active design system.
- Complete quoted `@plugin` paths and follow links to their resolved modules.
- Inspect colors and use color pickers in utilities and theme references.
- Apply compiler Quick Fixes or run **Stillwind: Sort Utilities**.

CSS at-rule completion normally offers application and theme authoring syntax.
Enable `stillwind.completion.registryDirectives` when editing utility libraries.
The project `preview` settings control theme values, pixel equivalents, and colors
shown in CSS previews. See [Editor previews](https://github.com/SegaraRai/stillwind/blob/main/docs/standalone-analysis.md#editor-previews)
for their options.

## Sort and fix on save

Use a document formatter such as Prettier or Oxfmt for layout and whitespace.
Enable Stillwind's Code Actions alongside your formatter:

```json
{
  "editor.formatOnSave": true,
  "editor.codeActionsOnSave": {
    "source.fixAll.stillwind": "explicit",
    "source.sort.stillwind": "explicit"
  }
}
```

`source.fixAll.stillwind` applies enabled lint fixes and respects suppression
comments. `source.sort.stillwind` sorts independently of lint severity.
`lint: false` disables lint rules and fixes. `sort: false` also disables explicit
sorting commands. Configure sorting targets in the project's `sort` field.

## Web workspaces

The Web extension requires a host with cross-origin isolation. Project configuration
can import browser-compatible modules; Node built-ins are unavailable. Workspace
dependencies must be readable through the workspace filesystem.
The default browser provider supports JS, JSX, TS, and TSX; component documents
require a custom source or projection provider.

See [project configuration and editor contracts](https://github.com/SegaraRai/stillwind/blob/main/docs/standalone-analysis.md)
for lint presets, stylesheet support, and sorting options.
