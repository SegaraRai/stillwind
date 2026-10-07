# Standalone analysis

Stillwind analysis reads source and semantic information without starting a bundler.
`@stillwind/cli` wraps sorting, lint, and stdio commands. `@stillwind/analysis` owns analysis
services, and `@stillwind/language-server` owns LSP handlers and Node/browser transports.
The LSP can run independently of the CLI.

Node configuration runs in an isolated worker. Configuration stdout goes to stderr,
so it cannot corrupt CLI JSON or LSP messages. Analysis does not evaluate application modules
or write generated vocabulary files.

## Shared project configuration

`defineConfig` is exported by `stillwind/config`. Each project directly selects its
compiler settings and diagnostic scope:

```ts
import { defineConfig } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      root: ".",
      files: ["src/**/*.{ts,tsx,vue,svelte,astro}"],
      exclude: ["src/**/*.generated.ts"],
      css: "src/theme.css",
      tsconfig: "tsconfig.app.json",
      classNamespace: "app",
      sources: { kind: "filesystem" },
      projection: { kind: "framework" },
    },
  ],
});
```

The root object contains only `projects`. Project roots are relative to this
configuration file. Stylesheets, tsconfig paths, and generated declaration paths
are relative to the selected project root. Omission selects the configuration
directory and an existing root `tsconfig.json`. `tsconfig: false` selects isolated
checker defaults. The source and projection providers shown above are also the defaults.

`defineConfig` also accepts a function returning settings, an async function, or a
Promise of settings. It returns the supplied export unchanged. The loader calls a
function once without arguments, awaits its result, and validates the resolved
settings. For example:

```ts
export default defineConfig(async () => ({
  projects: [{ css: "src/theme.css", files: ["src/**/*.tsx"] }],
}));
```

`files` and `exclude` select diagnostic targets. They do not restrict imports or
global declarations read by the checker. Target enumeration always excludes
`node_modules` and `.git`. Matching projects run independently, including overlapping
scopes. Results identify the configuration path and the project array index.

Build integrations discover the nearest `stillwind.config.ts` from the application root.
For Vite, use `stillwind()` without duplicating compiler settings:

```ts
// vite.config.ts
import { defineConfig } from "vite-plus";
import { stillwind } from "@stillwind/integration/vite";

export default defineConfig({ plugins: [stillwind()] });
```

Each integration keeps its bundler's plugin or wrapper form. webpack and Rspack use
`new StillwindPlugin()`, Rsbuild uses `pluginStillwind()`, and Next uses
`withStillwind(nextConfig)`. Standalone Turbopack uses `stillwindRules({ entries })`.
These entrypoints accept the same configuration discovery and inline override options.
Pass those options as the second argument to `withStillwind`.

`stillwind({ configFile: "config/stillwind.config.ts" })` selects another file, relative
to the application root. An explicit file must exist. `configFile: false` disables
file loading; supply a CSS entrypoint inline in that case. Other inline compiler
settings override the selected project's settings. Undefined preserves the file's
value, false remains explicit, and arrays replace previous arrays. Nested objects merge
recursively; changing a provider's kind replaces that provider.

Project `name` is optional. `stillwind({ project: "web" })` selects a matching name;
names must be unique. Without a name, a single project is selected automatically.
With several projects, the integration chooses the deepest project root containing
the application root (or the inline root override). No match or several matches at
that root produce a configuration error. Analysis continues to run every matching
project independently.

A root in the configuration file is relative to its directory. An inline root is
relative to the application root. CSS, tsconfig, and generated declaration paths
use the resulting project root. Analysis scope settings do not filter the bundler's
module graph. Build-specific settings such as Vite CSS emission stay in the
integration configuration.

`@stillwind/config` owns the shared definitions, export evaluation, validation, and
merge operation. `stillwind/config` re-exports that public API. `@stillwind/config/node`
owns discovery, trusted JS/TS evaluation, project selection, and path resolution.
Browser consumers can resolve an in-memory export through the shared entrypoint.

Vite restarts its server when a configuration dependency changes. Production watch
builds and webpack/Rspack compilations evaluate settings again before compiling.
Next and Turbopack pass configuration references to loaders; configuration callbacks
stay in the file rather than being serialized into loader options. Inline naming
callbacks remain unsupported by these loaders.

Lint levels remain in CSS:

```css
@import "stillwind";
@stillwind {
  prefer-typed-relationships: deny;
  unknown-utility: warn;
}
```

## Semantic and source services

The Node Type Facts backend resolves import and export requests from TypeScript module
symbols. It uses the selected tsconfig's paths, custom conditions, types, and JSX settings.
The same service retains an incremental snapshot for subsequent declaration queries.
CSS has a separate resolver with the `style` condition.

The checker project inherits the selected tsconfig and retains its declaration files.
Prepared sources, framework globals, and generated vocabulary declarations become its
other roots. The generated configuration clears inherited target include/exclude patterns.
Unsaved buffers override reads, including dependencies outside the diagnostic scope,
without becoming checker roots merely because they are open.

TypeScript may resolve an import to an ambient declaration or a separate `.d.ts` file.
Those files can supply ordinary types without supplying Stillwind producer bodies.
A custom `sources` provider can return implementation inputs or metadata for such
modules. Its `resolve` operation supplements checker resolution; its `load` operation
receives the captured document map. Inputs retain their compiler identity, checker
filename, original document, and projection metadata. A custom `projection.prepare`
operation can supply a prepared source. Both provider types use discriminated unions,
so custom operations are required only for their custom variants.

## Available and unverified analysis

Analysis keeps diagnostics determined from local source when the checker, an import,
or a required declaration body is unavailable. Results record the original UTF-8 ranges
and reasons in `graph.unverified` (or `unverified` in the portable service result).
`typeFacts: false` skips semantic queries explicitly. Incomplete lint returns exit code 2.

The native `diagnose` method collects local findings from the available IR and returns
only diagnostic batches. It does not generate or publish artifacts. Analysis omits
lowering errors caused by unavailable identities and retains the unverified ranges instead.
Known syntax errors and locally determined findings remain source diagnostics. When only
receiver recognition for lint is unverified, code generation can still complete.

`generation: "blocked"` marks output whose declaration-specific names or semantics could
not be verified. Imported ref projections and keyframes can require those identities
for CSS generation. Build compilation keeps its existing requirement for this information;
there is no universal runtime fallback for unresolved values. Unverified analysis does
not return usable CSS or code for a module whose generation requires the unavailable information.

## CLI and editor behavior

```sh
stillwind fmt
stillwind fmt --check
stillwind lint
stillwind lint --fix
stillwind lint 'src/**/*.tsx' --format json
stillwind lint --config ./tools/stillwind.config.ts
stillwind lsp --stdio
```

Without `--config`, the CLI searches upward from the working directory. The language
server searches from each workspace folder. CLI globs narrow the configured
scopes. Target enumeration includes files that the application never imports.
Each target's cross-module analysis follows the semantic imports needed by its
type probes; results report `coverage: "observed"`.

Lint exits with `0` after complete checks without error reports, `1` for compiler
errors or deny-level findings, and `2` for initialization failures, incomplete
analysis, or an empty target selection. Warnings preserve exit code `0`.

`fmt` sorts recognized Stillwind inputs independently of sorting lint severity and source lint
suppression. It leaves layout and utility spelling intact. `fmt --check` writes nothing and returns
`1` when sorting would change a file. Use `stillwind fmt && vp fmt` to apply sorting and general
formatting. The [Prettier plugin](../packages/prettier-plugin/README.md) performs sorting within
Prettier's parser pipeline.

`lint --fix` applies enabled automatic fixes until they converge and reports findings from the
final source buffers. Source lint suppression still applies. Both commands compare original and
final bytes and only save changed files. They preserve modification times for unchanged files.
Planning fails before saving when analysis is incomplete or overlapping projects disagree on
the replacement text. See the [CLI reference](../packages/cli/README.md) for saving and exit codes.

The language server keeps unsaved buffers for dependencies as well as targets.
Each project serializes analysis and reuses its checker. A changed revision
discards stale results. Document changes recheck open targets, including their
consumers. Saves and watched disk changes reopen services and reload their
configuration, CSS, and declarations. Clients supporting dynamic
watch registration receive a workspace file watcher registration.

Reports retain UTF-16 ranges, related labels, and document scope. Quick fixes are
offered only when every edited document is open and its native content revision
still matches. Edits carry LSP document versions. The server forwards actions
provided by the compiler; it does not invent replacements for findings without
actions. Initialization failures produce `SW_HOST` errors. Unavailable semantic checks produce
`SW_UNVERIFIED` information diagnostics, alongside the verified source findings.

## Embedding the language server

`createLanguageServer` from `@stillwind/language-server` registers handlers on a
caller-owned LSP connection. The caller supplies `openWorkspaces`, which returns
analysis services. Each service receives document URIs, versioned buffers, and an
optional cancellation signal. The common server preserves URI schemes such as
`inmemory:` and `vscode-vfs:`. It does not convert them to filesystem paths.

For a Node extension, the caller can select its own IPC or stream transport:

```ts
import { openNodeWorkspaces } from "@stillwind/analysis/node";
import { createLanguageServer } from "@stillwind/language-server";
import { createConnection, ProposedFeatures } from "vscode-languageserver/node";

const connection = createConnection(ProposedFeatures.all);
const server = createLanguageServer(connection, {
  openWorkspaces: openNodeWorkspaces,
  onExit: (shutdown) => process.exit(shutdown ? 0 : 1),
});
connection.listen();
```

`@stillwind/language-server/node` also exports `startLanguageServer(config?)` for
the stdio transport. The CLI delegates its LSP command to this entrypoint.
`@stillwind/analysis/node` exports `format`, `lint`, and `formatLint` for callers that do not
need a command-line parser. `SourceFormatter` sorts unsaved buffers without saving them and
refreshes configuration for each request.

For a Web Extension or a static editor site, start the browser entrypoint in a
Worker. It also accepts a MessagePort. `BrowserAnalysisWorkspace` from
`@stillwind/analysis/browser` supplies the compiler and semantic checker for a
captured virtual filesystem. Include source files, package metadata, dependency
declarations, and TypeScript standard libraries in that inventory. Its project
roots use absolute virtual paths. Open documents replace captured file text.

`loadBrowserConfig` from `@stillwind/config/browser` loads every configured project.
The embedding supplies an evaluator for browser-compatible configuration modules.
`resolveBrowserConfig` selects one project and applies inline overrides when a
build integration needs that selection.

The public [`@stillwind/vscode-extension`](../packages/vscode-extension/README.md)
package supplies Desktop and Web clients. Its Web server evaluates configuration,
reads workspace files through VS Code's filesystem provider, and runs the compiler
and browser checker in a worker. The static [Playground](../apps/play/README.md)
uses this extension alongside the built-in TypeScript language extension.

An embedding with its own compiler callback can instead use the memory service:

```ts
import { createMemoryAnalysisWorkspace } from "@stillwind/analysis/memory";
import { startBrowserLanguageServer } from "@stillwind/language-server/browser";

startBrowserLanguageServer(self, {
  openWorkspaces: async () => {
    const { compileSource } = await import("@stillwind/native");
    return {
      workspaces: new Map([
        [
          "preview",
          createMemoryAnalysisWorkspace({
            config: "inmemory://preview/stillwind.config.ts",
            includes: (uri) => uri.endsWith(".tsx"),
            compile: async ({ uri, documents }) => {
              const output = compileSource({
                source: documents.get(uri) ?? "",
                moduleId: uri,
                uri,
                sourceMapSource: uri,
                language: "tsx",
                moduleKind: "unambiguous",
                css: {
                  content: "@utility flex{display:flex}@stillwind-slot utilities;",
                  name: "theme.css",
                  uri: "inmemory://preview/theme.css",
                  sourceMapSource: "inmemory://preview/theme.css",
                },
              });
              return { status: "complete", reports: output.reports };
            },
          }),
        ],
      ]),
    };
  },
  onExit: () => self.close(),
});
```

This example checks one TSX document with literal utilities through the WASM
compiler. The embedding application supplies its module loader, component
projections, and TypeScript facts for cross-module or typed analysis. It can
connect its TS Server worker through this compiler callback. The existing Node
checker uses TypeScript's native process API and cannot run in a browser.

The memory service captures all open documents, including dependencies, before
calling the compiler. It serializes calls and suppresses stale or cancelled
results. It passes the cancellation signal to the compiler. Browser services
receive already-loaded configuration and compiler resources; they do not
evaluate Vite configuration or start Node processes. The WASM runtime requires
the same isolation headers as other uses of `@stillwind/native` in a browser.

Call `dispose()` on the returned server when the embedding application releases
it. The common server closes analysis services. The browser entrypoint also
disposes its connection and readers. The application owns the Worker or MessagePort
and terminates or closes it separately.

The host APIs are also usable directly: `AnalysisWorkspace.open` reads
configuration, while `AnalysisProject` owns source services and an optional semantic backend.
`createFileSystemAnalysisHost` is exported from `@stillwind/host`.
The `TypeFactsBackend` contract is exported as a type by `@stillwind/analysis` and
`@stillwind/host`. Its operations may be asynchronous so an embedding
application can use a checker worker. `TypeScriptFactsService` uses the Node
TypeScript backend. `BrowserTypeFactsService` from `@stillwind/host/browser` uses
the JavaScript TypeScript checker against virtual files. `BrowserProject` and
`BrowserAnalysisWorkspace` use it for cross-module declaration identities and
semantic dependencies. The Playground's separate TS Server provides ordinary
editor completions, navigation, and TypeScript diagnostics.
Call `close()` when releasing these objects.

## Editor sorting

`sort.arrays` groups direct arrays, `style()` argument lists, and object entries by input kind. It defaults to true. Ref groups retain their first appearance order, with each ref's marker before its projections. Bases, utilities, custom properties, and attributes follow. Entries in each category retain their original order. A base can contain variants, custom properties, or composed styles; the sorter uses its identity without inspecting its body or CSS.

`sort.stringLiterals` defaults to true and sorts utility tokens inside each static string or template segment using compiler order. This does not reorder separate utility array elements. Unknown utility tokens delimit sortable string runs.

`sort.attributes` defaults to false. Set it to true to sort adjacent `aria-` and `data-` property names. It works independently of the other two options. Duplicate names keep their original order. Spreads, unsupported expressions, ambiguous comments, and unmapped source regions delimit sortable input runs. Explicit sorting is available when sorting diagnostics are off; lint fixes respect source suppression.

```ts
import { defineConfig } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "theme.css",
      files: ["src/**/*.tsx"],
      sort: { arrays: true, stringLiterals: true, attributes: true },
    },
  ],
});
```
