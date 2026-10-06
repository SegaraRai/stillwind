# Stillwind for Visual Studio Code

This extension starts the Stillwind language server independently of the CLI. Desktop VS Code uses a Node process. Web VS Code uses a worker, a virtual workspace, and the WASM compiler.

Add `stillwind.config.ts` to your workspace. Set `stillwind.configFile` to select another file. The extension reports Stillwind diagnostics and supplies Quick Fixes from the compiler. `Stillwind: Restart Language Server` reloads configuration and dependency files.

The Web extension requires cross-origin isolation: serve `Cross-Origin-Opener-Policy: same-origin` and `Cross-Origin-Embedder-Policy: require-corp`. Browser configuration can import browser-compatible modules. Imports of Node built-ins fail during configuration evaluation.

The Web bundle includes TypeScript standard libraries and fallback Stillwind CSS, declarations, and configuration helpers. Installed workspace packages take precedence. Other dependencies must be readable through the workspace filesystem. Component projections require a custom source or projection provider; the default browser provider handles JS, JSX, TS, and TSX.

Run `vp run package:web` to create the Web VSIX. Run `vp run package:desktop` to create a VSIX for the current operating system and architecture. Both commands use the shared manifest and extension sources; neither publishes to a marketplace.
