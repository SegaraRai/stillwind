# Playground formatting

The static Playground uses Prettier for full-document formatting. The embedded extension `stillwind-internal.prettier` owns the formatter provider for JavaScript, JSX, TypeScript, TSX, CSS, JSON, and JSONC. Language-specific `editor.defaultFormatter` settings select it. The editor's Format button requests `source.sort.stillwind` actions for the whole buffer, awaits the sort edit, and then runs the standard Format Document action. If different projects offer conflicting sorts, the button reports an error so the user can choose a Code Action. If no sort applies, the button still formats the document. The standard Format Document command runs only the selected formatter.

The [Prettier standalone runtime](https://prettier.io/docs/browser) runs without Node.js. Parser plugins are bundled into static chunks and loaded when their language is formatted. Formatting uses the buffer's indentation settings and Prettier defaults. It does not discover configuration or ignore files or load third-party plugins. Cancelled requests and requests whose buffer version changed produce no edits. Invalid syntax uses VS Code's formatter error handling.

The [official Oxc extension](https://github.com/oxc-project/oxc-vscode/blob/main/package.json) has a Node entry point and starts the Oxfmt binary. It has no browser entry point. The Oxc project also [discusses the missing official browser build](https://github.com/oxc-project/oxc/discussions/25092). Community WASM adapters exist, including [dprint's Oxc adapter](https://github.com/dprint/dprint-plugin-oxc), but adopting one would add a separate formatter integration. Prettier provides the runtime needed by this static site directly.

Stillwind sorts utility inputs and applies lint fixes. These operations need project analysis and theme CSS, and they preserve the rest of the document's layout. The LSP provides them through `source.sort.stillwind` and `source.fixAll.stillwind`. The extension exposes Stillwind: Sort Utilities through the same Code Action. Document formatting stays with the selected general formatter, so selecting Prettier does not disable Stillwind actions.

The upstream examples support this separation:

- [Tailwind CSS IntelliSense](https://github.com/tailwindlabs/tailwindcss-intellisense#extension-commands) exposes Sort Selection as a dedicated command. [Its Prettier plugin](https://github.com/tailwindlabs/prettier-plugin-tailwindcss) integrates sorting with a general formatter.
- [VS Code ESLint](https://github.com/microsoft/vscode-eslint#settings-options) provides `source.fixAll.eslint` for saved-file fixes. Its [formatter registration is opt-in](https://github.com/microsoft/vscode-eslint/blob/main/package.json), with `eslint.format.enable` defaulting to false.

A Stillwind formatter plugin could eventually combine both operations in one formatter invocation. It would need to accept the active project and theme and preserve the existing edit contracts. The Playground's Format button composes the existing source action and formatter without changing their registration.
