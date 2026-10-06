# Stillwind for Visual Studio Code

This extension starts the Stillwind language server independently of the CLI. Desktop VS Code uses a Node process. Web VS Code uses a worker, a virtual workspace, and the WASM compiler.

Add `stillwind.config.ts` to your workspace. Set `stillwind.configFile` to select another file. The extension reports Stillwind diagnostics and supplies Quick Fixes from the compiler. `Stillwind: Restart Language Server` reloads configuration and dependency files.

Hover over a utility, `style()` call, or its bound variable to see the generated CSS with the main selector written as `&`. Completion includes utilities, variants, and theme values from the active design system. Completion details load CSS when the selected item is resolved. Concrete colors have inline swatches and color picker edits.

`Stillwind: Sort Utilities` and document formatting sort strings and safe array elements. Refs precede base styles, and base styles precede utility strings. Dynamic expressions and unresolved entries separate runs. Base styles keep their relative order when their properties can interfere. Comments and duplicate entries are retained.

Canonical utility lint defaults to `warn`. Pixel spacing can become rem-based utilities when the active spacing scale uses rem; other domains, such as border width, retain their units. `source.fixAll.stillwind` applies enabled lint fixes. `source.sort.stillwind` sorts independently of lint severity. Sorting diagnostics are disabled by default.

```ts
import { defineConfig } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      files: ["src/**/*.tsx"],
      css: "src/theme.css",
      lint: {
        "canonical-utilities": {
          pxToRem: true,
          rootFontSize: 16,
        },
        "sort-utilities": "off",
      },
      sort: {
        stringLiterals: true,
        arrays: true,
      },
    },
  ],
});
```

Project lint fields override only the fields supplied. CSS severity directives and source suppression comments also apply. Hover, completion, colors, and edit plans are shared by Desktop and Web VS Code and the static Playground.

The Web extension requires cross-origin isolation: serve `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`. Browser configuration can import browser-compatible modules. Imports of Node built-ins fail during configuration evaluation.

The Web bundle includes TypeScript standard libraries and fallback Stillwind CSS, declarations, and configuration helpers. Installed workspace packages take precedence. Other dependencies must be readable through the workspace filesystem. Component projections require a custom source or projection provider; the default browser provider handles JS, JSX, TS, and TSX.

Run `vp run package:web` to create the Web VSIX. Run `vp run package:desktop` to create a VSIX for the current operating system and architecture. Both commands use the shared manifest and extension sources; neither publishes to a marketplace.
