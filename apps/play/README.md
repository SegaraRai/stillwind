# Stillwind Play

The playground serves static files. Monaco uses VS Code services and the built-in TypeScript extension. The public Stillwind Web extension provides compiler diagnostics and Quick Fixes. Application builds run in a separate worker with esbuild-wasm. React Refresh updates a persistent sandboxed iframe and retains compatible component state. CSS updates replace the stylesheet in that frame.

Run `vp run dev` for development. Run `vp run build` and serve `dist/site` to deploy. Configure `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp` on the static host. The generated `_headers` file uses the Netlify/Cloudflare Pages format. Hosting that cannot provide these headers cannot run the threaded WASM compiler.

The initial workspace contains `Component.tsx`, an imported style module, a stylesheet, `stillwind.config.ts`, and `tsconfig.json`. Add local source files through the file pane. All runtime dependencies and React declarations are bundled at site build time. Configuration supports objects, functions, async functions, and promises. Browser configurations can use browser-compatible imports; Node built-ins are unavailable.

Prettier is the default formatter for JS, JSX, TS, TSX, CSS, JSON, and JSONC. The editor's **Format** button applies Stillwind sorting before running Prettier. The standard **Format Document** command runs Prettier alone. Its standalone runtime and parser plugins are bundled with the site and use the editor's indentation settings. Prettier configuration files and third-party plugins are not loaded. Stillwind utility sorting and lint fixes are also available through `Stillwind: Sort Utilities`, `source.sort.stillwind`, and `source.fixAll.stillwind`. Cancelled requests and requests for an older buffer version produce no edits. Conflicting sorts from different projects are reported instead of choosing one automatically.

The preview entry exports `App` or a default React component from `Component.tsx`. Changes that alter hook order can reset component state under React Refresh. Build errors retain the last successful preview.
