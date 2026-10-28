# Playground development

The playground serves static files. Monaco uses VS Code services and the built-in
TypeScript extension. The public Stillwind Web extension supplies compiler diagnostics
and Quick Fixes. Application builds run separately in an esbuild-wasm worker.
React Refresh updates a persistent sandboxed iframe; CSS updates replace its stylesheet.

## Local tasks and hosting

Run from the repository root:

```sh
vp run @stillwind-internal/apps-play#dev
vp run @stillwind-internal/apps-play#build
```

Serve `dist/site` with `Cross-Origin-Opener-Policy: same-origin` and
`Cross-Origin-Embedder-Policy: require-corp`. The generated `_headers` file uses the
Netlify/Cloudflare Pages format. The threaded WASM compiler requires these headers.

## Workspace and preview

The initial workspace contains `Component.tsx`, an imported style module, a stylesheet,
`stillwind.config.ts`, and `tsconfig.json`. Its CSS loads the Iconify plugin, and the
workspace bundles the `mdi`, `lucide`, and `fluent-emoji-flat` icon sets.
The explorer can add local source files. Site builds bundle runtime dependencies
and React declarations.

The result pane shows the preview, the CSS that Stillwind generated for the file open in
the editor, that file's compiled code, and the bundled stylesheet of the preview build. A
read-only Monaco editor shows the outputs. Its models are not sent to extensions, so language
servers do not report diagnostics for them. The preview frame stays mounted while another view
is shown. The Problems panel below the editor lists diagnostics from all workspace files.

Configuration accepts objects, functions, async functions, and promises. Imports
must be browser-compatible; Node built-ins are unavailable.
See [Web extension development](../../packages/vscode-extension/DEVELOPMENT.md)
for the shared editor runtime and analysis embedding.

The preview entry exports `App` or a default React component from `Component.tsx`.
Compatible edits retain component state. Changes to hook order can reset it.
Build errors retain the last successful preview.

## Formatting and edits

Prettier formats JS, JSX, TS, TSX, CSS, JSON, and JSONC with the editor's indentation
settings. Format Document and the Format button run the same formatter. For JS, TS, and
CSS, the browser entry of `prettier-plugin-stillwind` sorts Stillwind utilities inside
Prettier, and the Web extension's `stillwind.sortSource` command computes the sort edits.
The site bundles Prettier's standalone runtime and parsers. It does not load Prettier
configuration files or other plugins.

The Fix button applies `source.fixAll.stillwind` to the file open in the editor. It is
disabled when that file has no fixable Stillwind lint findings. Stillwind sorting and fixes
are also available through `Stillwind: Sort Utilities`, `source.sort.stillwind`, and
`source.fixAll.stillwind`. Cancelled requests and older buffer versions produce no edits.
Conflicting sorts or fixes from different projects are reported without selecting one
automatically.

## Interface styles

The playground interface uses Stillwind through the Vite integration. `stillwind.config.ts`
selects `src/styles.css` for `src/**/*.tsx`. `src/theme.css` uses the website's token
names, type scale, and fonts with its dark palette. Monaco reads its colors and code font
from these tokens when the workspace opens. `src/icons/stillwind` contains the logo icons
from the website and the colored mark for `stillwind.config.ts`.
