# VS Code extension development

See [repository setup](../../DEVELOPMENT.md) for workspace prerequisites. The shared
manifest and extension sources produce both Desktop and Web packages.
`build.ts` copies the user README and license into the extension output.

## Runtime and browser embedding

Desktop VS Code runs the language server in a Node process. Web VS Code uses a
worker, a virtual workspace, and the WASM compiler. The Web bundle includes
TypeScript standard libraries and fallback Stillwind CSS, declarations, and
configuration helpers. Installed workspace packages take precedence.

Browser hosts must serve `Cross-Origin-Opener-Policy: same-origin` and
`Cross-Origin-Embedder-Policy: require-corp`. Rolldown bundles configuration from
the captured workspace before evaluation. Imports of Node built-ins fail during
configuration evaluation.

Desktop, Web VS Code, and the static Playground share hover, completion, colors,
and edit plans through the language server. The Web extension also registers hidden
commands for the Playground. `stillwind.snapshot` captures workspace files.
Its `metadataOnly` option defers text except package manifests. `stillwind.readFile`
reads requested file text, and `stillwind.compile` transforms application sources.
Reads and compilation require the snapshot's generation and revision.
`stillwind.sortSource` returns sort edits for a buffer that the
browser entry of `prettier-plugin-stillwind` formats. `stillwind.sortSource` rejects
text that differs from the server's current buffer. Transport and analysis embedding
contracts are maintained in [Standalone analysis](../../docs/standalone-analysis.md#embedding-the-language-server).

## Build and package

Run from the repository root:

```sh
vp run @stillwind/vscode-extension#build
vp run @stillwind/vscode-extension#package:desktop
vp run @stillwind/vscode-extension#package:web
```

The build bundles the extension and workers with Vite+ Pack and copies the WASM
runtimes and libraries. Desktop packaging targets the current operating system
and architecture. The outputs are `dist/stillwind-desktop.vsix` and
`dist/stillwind-web.vsix`. Packaging does not publish them to a marketplace.

## Verification

```sh
vp run @stillwind/vscode-extension#test:browser
vp run @stillwind/vscode-extension#test:package
```

The browser suite checks configuration bundling with packaged WASM in Chrome.
The package suite checks the standalone Desktop server.
