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
The file pane can add local source files. Site builds bundle runtime dependencies
and React declarations.

Configuration accepts objects, functions, async functions, and promises. Imports
must be browser-compatible; Node built-ins are unavailable.
See [Web extension development](../../packages/vscode-extension/DEVELOPMENT.md)
for the shared editor runtime and analysis embedding.

The preview entry exports `App` or a default React component from `Component.tsx`.
Compatible edits retain component state. Changes to hook order can reset it.
Build errors retain the last successful preview.

## Formatting and edits

Prettier formats JS, JSX, TS, TSX, CSS, JSON, and JSONC with the editor's indentation
settings. The Format button runs Stillwind sorting before Prettier; Format Document
runs Prettier alone. The site bundles Prettier's standalone runtime and parsers.
It does not load Prettier configuration files or third-party plugins.

Stillwind sorting and fixes are also available through `Stillwind: Sort Utilities`,
`source.sort.stillwind`, and `source.fixAll.stillwind`. Cancelled requests and older
buffer versions produce no edits. Conflicting sorts from different projects are
reported without selecting one automatically.
