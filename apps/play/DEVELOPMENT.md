# Playground development

The playground is a static Astro site. `src/pages/index.astro` renders the document head,
including the title, description, canonical URL, and Open Graph and Twitter card tags, and it
mounts `src/Playground.tsx` with `client:only="react"`. The playground reads browser state while
rendering, so the build emits no HTML for it. Monaco uses VS Code services and the built-in
TypeScript extension. The public Stillwind Web extension supplies compiler diagnostics
and Quick Fixes. Application builds run separately in a worker with the browser build
of Rolldown. Its JSX transform adds React Refresh registrations, and React Refresh updates a
persistent sandboxed iframe. CSS updates replace its stylesheet.

Rolldown does not bundle CSS. The build inlines `@import` rules that name workspace
stylesheets, and it appends imported stylesheets in module order after the Stillwind CSS.
Other `@import` rules and `url()` references stay unchanged.

## Local tasks and hosting

Run from the repository root:

```sh
vp run @stillwind-internal/apps-play#dev
vp run @stillwind-internal/apps-play#build
```

`build/assets.ts` writes the static assets and the files from `public/` to
`dist/assets-static`, which Astro uses as its public directory. Serve `dist/site` with
`Cross-Origin-Opener-Policy: same-origin` and
`Cross-Origin-Embedder-Policy: require-corp`. The generated `_headers` file configures
these headers for Cloudflare Workers Static Assets. The threaded WASM compiler requires them.

See [Site deployments](../../DEVELOPMENT.md#site-deployments) for CI, Preview,
production, and the required Cloudflare and GitHub settings.

`PUBLIC_PLAYGROUND_URL` sets the origin of canonical URLs and social cards.
`PUBLIC_WEBSITE_URL` sets links to the website and its documentation, including the
404 page. They default to `https://play.stillwind.dev/` and `https://stillwind.dev/`.
The build and content-sync task caches include both variables.

## Workspace and preview

The workspace starts from a preset. Each preset lives in `examples/<id>/`, and
`src/workspace/presets.ts` lists the presets in menu order. Vite embeds their files as text when
it builds the site. Every preset needs `App.tsx`, which exports `App` or a default component.

- `minimal` is the default. It keeps a few utilities and role-based tokens, so a reproduction
  can start from it without deleting code.
- `release-checklist` shows recommended Stillwind usage: role-based tokens with `light-dark()`,
  a custom variant, custom utilities, refs, container queries, view transitions, and lint rules
  that report values outside the theme.

The workspace bundles the `mdi`, `lucide`, and `fluent-emoji-flat` icon sets.

Presets must have no Stillwind lint findings and must keep Prettier's default formatting and
Stillwind's utility order, so the Format and Fix buttons leave them unchanged. `vp fmt` skips
`examples/` because its settings differ from Prettier's defaults. Check or fix the presets with:

```sh
vp run @stillwind-internal/apps-play#check:examples
vp run @stillwind-internal/apps-play#fmt:examples
```

The repository `check` and `fmt` tasks include them. `e2e/play` opens `release-checklist` and
edits its files by their text, so update its replacements when that preset changes.

## Presets and permalinks

The location fragment selects the workspace. `#preset=<id>` opens a preset, and an empty
fragment opens the first one. The Share button writes `#code=<data>` and copies the URL.
The data is deflate-compressed JSON in base64url. It contains every editable file, the preset
id, the file open in the editor, and the Stillwind version of the site. A link therefore
reproduces its files even after the preset changes, and the playground shows a notice when the
version differs. Unknown presets and unreadable links open the first preset with a notice.

Choosing a preset or Reset reloads the page with `#preset=<id>`. Both ask for confirmation when
the files differ from the preset.

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

The preview entry exports `App` or a default React component from `App.tsx`.
Compatible edits retain component state. Changes to hook order can reset it.
Build errors retain the last successful preview.

The preview frame uses `sandbox="allow-scripts allow-forms"` without `allow-same-origin`, so
the application cannot reach the editor. The frame must keep its document, so the runtime
cancels form submissions and clicks on links that would load another document. Submit and
click handlers still run, and a `#id` link scrolls to its target. The opaque origin denies
`localStorage` and `sessionStorage`, so the runtime replaces them with storage that lasts
until the frame reloads. IndexedDB, cookies, `alert()`, popups, and the clipboard API remain
unavailable.

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

The playground interface uses Stillwind through the Astro integration, which adds the
stylesheet to the page. `stillwind.config.ts` selects `src/styles.css` for `src/**/*.tsx` and
`src/**/*.astro`. `src/theme.css` uses the website's token
names, type scale, fonts, and light and dark palettes with `light-dark()`.

The theme button cycles through system, light, and dark. The playground stores the choice
under the website's `stillwind:theme` local storage key and sets `data-theme` on the root
element, and an inline script in `src/pages/index.astro` applies the stored choice before the
first paint. Monaco uses the
Default Light Modern or Default Dark Modern theme with colors resolved from the tokens, and
the playground updates them when the color scheme changes. `src/icons/stillwind` contains the logo icons
from the website and the colored mark for `stillwind.config.ts`.
