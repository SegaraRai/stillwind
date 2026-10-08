# Stillwind for Visual Studio Code

This extension starts the Stillwind language server independently of the CLI. Desktop VS Code uses a Node process. Web VS Code uses a worker, a virtual workspace, and the WASM compiler.

Add `stillwind.config.ts` to your workspace. Set `stillwind.configFile` to select another file. The extension reports Stillwind diagnostics and supplies Quick Fixes from the compiler. `Stillwind: Restart Language Server` reloads configuration and dependency files.

Hover over a utility, `style()` call, or its bound variable to see the generated CSS with the main selector written as `&`. Completion includes utilities, variants, and theme values from the active design system. Completion details load CSS when the selected item is resolved. Concrete colors have inline swatches and color picker edits.

`Stillwind: Sort Utilities` invokes the `source.sort.stillwind` Code Action. Utility strings use compiler ordering. Arrays, argument lists, and object properties use stable groups: each ref's markers then projections, bases, utilities, custom properties, and attributes. Entries within each group keep their authored order. Unsupported expressions and comments separate runs. Set `sort.attributes` to sort aria-/data- attribute names independently; it defaults to false.

Use a document formatter such as Prettier or Oxfmt for layout and whitespace. Stillwind provides sorting and lint fixes as Code Actions. To run them on explicit saves alongside your formatter, configure VS Code:

```json
{
  "editor.formatOnSave": true,
  "editor.codeActionsOnSave": {
    "source.fixAll.stillwind": "explicit",
    "source.sort.stillwind": "explicit"
  }
}
```

Canonical utility lint defaults to `warn`. Pixel spacing can become rem-based utilities when the active spacing scale uses rem; other domains, such as border width, retain their units. `source.fixAll.stillwind` applies enabled lint fixes. `source.sort.stillwind` sorts independently of lint severity. Sorting diagnostics are disabled by default.

```ts
import { defineConfig, lintPresets } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      files: ["src/**/*.tsx"],
      css: "src/theme.css",
      lint: {
        extends: [lintPresets.recommended],
        rules: {
          "canonical-utilities": {
            pxToRem: true,
            rootFontSize: 16,
          },
          "sort-utilities": "off",
        },
      },
      sort: {
        stringLiterals: true,
        arrays: true,
      },
    },
  ],
});
```

Project lint rules override the selected presets. Source suppression comments also apply. Hover, completion, colors, and edit plans are shared by Desktop and Web VS Code and the static Playground.

The Web extension requires cross-origin isolation: serve `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`. Browser configuration can import browser-compatible modules. Imports of Node built-ins fail during configuration evaluation.

The Web bundle includes TypeScript standard libraries and fallback Stillwind CSS, declarations, and configuration helpers. Installed workspace packages take precedence. Other dependencies must be readable through the workspace filesystem. Component projections require a custom source or projection provider; the default browser provider handles JS, JSX, TS, and TSX.

Run `vp run package:web` to create the Web VSIX. Run `vp run package:desktop` to create a VSIX for the current operating system and architecture. Both commands use the shared manifest and extension sources; neither publishes to a marketplace.
