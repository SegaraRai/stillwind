# @stillwind/language-server

Stillwind compiles Tailwind CSS v4 utilities into component props and static CSS.
It uses theme rules and compiler checks to keep styling code in order, even when
AI agents write much of it.

`@stillwind/language-server` provides Stillwind's Language Server Protocol
handlers and Node/browser transports. The CLI and VS Code extension use these
services for diagnostics and editor queries.

See [Embedding the language server](https://github.com/SegaraRai/stillwind/blob/main/docs/standalone-analysis.md#embedding-the-language-server)
for connection ownership and analysis providers.
