# Stillwind for Visual Studio Code

This extension starts the Stillwind language server independently of the CLI. Desktop VS Code uses a Node process. Web VS Code uses a worker, a virtual workspace, and the WASM compiler.

Add `stillwind.config.*` to your workspace (`ts`, `mts`, `cts`, `js`, `mjs`, `cjs`, or `json`). Set `stillwind.configFile` to select another file. The extension reports Stillwind diagnostics and supplies Quick Fixes from the compiler. `Stillwind: Restart Language Server` reloads configuration and dependency files.

Hover over a utility, `style()` call, or its bound variable to see the generated CSS with the main selector written as `&`. Completion includes utilities, variants, and theme values from the active design system. Completion details load CSS when the selected item is resolved. Concrete colors have inline swatches and color picker edits.

Previews add comments after values: `var(--color-brand) /* oklch(63.7% 0.237 25.331) = #fb2c36 */`, `calc(var(--spacing) * 4) /* 1rem = 16px */`, and `1rem /* 16px */`. The `preview` field of a project selects these comments. `themeValues` adds resolved theme values and evaluated `calc()` results, `pixelEquivalents` adds pixel values for rem, and `colorEquivalents` adds hexadecimal sRGB colors. All three are enabled by default. `colorEquivalents: "approximate"`, the default, maps a color outside the sRGB gamut into the gamut and marks the result with `≈`. `"exact"` omits those colors, and `false` omits all color comments. Numbers in comments are rounded to six fractional digits; `≈` marks a rounded number, such as `calc(1.25 / 0.875) ≈ 1.428571`. A `calc()` that evaluates to one value receives one comment after the expression. When the expression combines rem with px, the comment contains the pixel total, such as `calc(var(--spacing) + 1px) /* 5px */`; this total requires `pixelEquivalents`. Other expressions receive comments on the theme references inside them.

`Stillwind: Sort Utilities` invokes the `source.sort.stillwind` Code Action. Utility strings use compiler ordering. Arrays, argument lists, and object properties use stable groups: each ref's markers then projections, bases, utilities, custom properties, and attributes. Entries within each group keep their authored order. Unsupported expressions and comments separate runs. Set `sort.attributes` to sort aria-/data- attribute names independently; it defaults to false.

CSS uses `sort.applyUtilities` to sort arguments inside `@apply` by default. Set
`sort.applyStatements` or `sort.variantBlocks` to sort adjacent statements or applied variant
blocks as well. Both default to false. Movement preserves the order of competing property writes.
CSS color decorators cover `@apply` candidates and active theme references in `theme()`, `--theme()`,
and `var()`. Picker edits preserve utility variants and modifiers. Editing a theme reference
replaces its current use with a color literal without editing the referenced declaration.

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

Canonical utility lint defaults to `warn`. Pixel spacing can become rem-based utilities when the active spacing scale uses rem; other domains, such as border width, retain their units. The project's `rootFontSize` sets the number of pixels per rem for this conversion and for preview comments. It defaults to 16. Media and container query comments always use 16 because browsers evaluate rem there against the initial font size. `source.fixAll.stillwind` applies enabled lint fixes. `source.sort.stillwind` sorts independently of lint severity. Sorting diagnostics are disabled by default. Set `lint: false` to disable all lint rules and fixes. Set `sort: false` to disable sorting, including explicit sorting commands.

```ts
import { defineConfig, lintPresets } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      files: ["src/**/*.tsx"],
      css: "src/theme.css",
      rootFontSize: 16,
      lint: {
        extends: [lintPresets.recommended],
        rules: {
          "prefer-canonical-utilities": {
            pxToRem: true,
          },
          "prefer-sorted-utilities": "off",
        },
      },
      preview: {
        colorEquivalents: "exact",
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

The Web extension requires cross-origin isolation: serve `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`. Browser configuration can import browser-compatible modules. Rolldown bundles configuration from the captured workspace before evaluation. Imports of Node built-ins fail during configuration evaluation.

The Web bundle includes TypeScript standard libraries and fallback Stillwind CSS, declarations, and configuration helpers. Installed workspace packages take precedence. Other dependencies must be readable through the workspace filesystem. Component projections require a custom source or projection provider; the default browser provider handles JS, JSX, TS, and TSX.

Run `vp run build` to bundle the extension and its workers with Vite+ Pack and copy the WASM runtimes and libraries. Run `vp run package:web` to create the Web VSIX. Run `vp run package:desktop` to create a VSIX for the current operating system and architecture. Both packaging commands use the shared manifest and extension sources; neither publishes to a marketplace.

Run `vp run test:browser` to check configuration bundling with the packaged WASM runtime in Chrome. Run `vp run test:package` to check the standalone Desktop server.
