# VS Code extension development

See [repository setup](../../DEVELOPMENT.md) for workspace prerequisites. The shared
manifest and extension sources produce both Desktop and Web packages.
`build.ts` copies the user README, license, and `icon.png` into the extension output.
`tools/brand` generates `icon.png`.

## Runtime and browser embedding

Desktop VS Code runs the language server in a Node process. The extension resolves
`@stillwind/cli` from the directory of `stillwind.configFile`, then from each
workspace folder in order, and starts `stillwind lsp --stdio`. It starts the bundled
server when no directory resolves the CLI or the CLI fails to initialize. One client
serves all workspace folders. Workspace Trust must allow the extension before it
runs a project's CLI.

Web VS Code cannot start a process. It uses a worker, a virtual workspace, and the
WASM compiler from the extension. The Web bundle includes TypeScript standard libraries
and fallback Stillwind CSS, declarations, and configuration helpers. Installed workspace
packages take precedence.

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
`stillwind.snapshot` accepts a `config` option that selects a workspace, and it returns the
`config` of the captured workspace. Reads and compilation accept the same option. Omitting
`config` requires exactly one open workspace, and the number of open workspaces can grow when
language server discovery opens the configuration of another package.
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

## Publish

The [Release](../../.github/workflows/release.yml) workflow builds the Web package and
one Desktop package for each of `win32-x64`, `win32-arm64`, `darwin-x64`,
`darwin-arm64`, `linux-x64`, and `linux-arm64`. It tests the server in each Desktop
package.

After every build and test job succeeds, the workflow publishes to the Visual Studio
Marketplace and Open VSX. Both registries publish immediately, and their versions are
immutable. A rerun skips versions that already exist.

The `extension-release` environment contains `OVSX_PAT`, an Open VSX token that can
publish to the `stillwind` namespace. The Marketplace publisher `stillwind` needs a
Trusted Publishing policy for the workflow `release.yml`, the repository
`SegaraRai/stillwind`, and that environment.

## Verification

```sh
vp run @stillwind/vscode-extension#test:browser
vp run @stillwind/vscode-extension#test:package
```

The browser suite checks configuration bundling with packaged WASM in Chrome.
The package suite checks the standalone Desktop server.
