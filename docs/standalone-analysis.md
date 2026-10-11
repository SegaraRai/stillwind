# Standalone analysis

Stillwind analysis reads source and semantic information without starting a bundler.
`@stillwind/cli` wraps sorting, lint, and stdio commands. `@stillwind/analysis` owns analysis
services, and `@stillwind/language-server` owns LSP handlers and Node/browser transports.
The LSP can run independently of the CLI.

Node configuration runs in an isolated worker. Configuration stdout goes to stderr,
so it cannot corrupt CLI JSON or LSP messages. Analysis does not evaluate application modules.

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
      sources: {
        kind: "filesystem",
      },
      projection: {
        kind: "framework",
      },
    },
  ],
});
```

The root object contains only `projects`. Project roots are relative to this
configuration file. Stylesheet and tsconfig paths are relative to the selected
project root. Omission selects the configuration
directory and an existing root `tsconfig.json`. `tsconfig: false` selects isolated
checker defaults. The source and projection providers shown above are also the defaults.

`defineConfig` also accepts a function returning settings, an async function, or a
Promise of settings. It returns the supplied export unchanged. The loader calls a
function once without arguments, awaits its result, and validates the resolved
settings. For example:

```ts
export default defineConfig(async () => ({
  projects: [
    {
      css: "src/theme.css",
      files: ["src/**/*.tsx"],
    },
  ],
}));
```

`files` and `exclude` select diagnostic targets. They do not restrict imports or
global declarations read by the checker. Target enumeration always excludes
`node_modules` and `.git`. Matching projects run independently, including overlapping
scopes. Results identify the configuration path and the project array index.

Build integrations discover the nearest `stillwind.config.*` from the application root.
Each directory is checked in this order: `ts`, `mts`, `cts`, `js`, `mjs`, `cjs`, and `json`.
A configuration in a nearer directory takes precedence over all files in its parents.
JavaScript and TypeScript files export settings, a promise, or a factory. JSON files
contain the settings object directly.
For Vite, use `stillwind()` without duplicating compiler settings:

```ts
// vite.config.ts
import { defineConfig } from "vite-plus";
import { stillwind } from "@stillwind/integration/vite";

export default defineConfig({
  plugins: [stillwind()],
});
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
relative to the application root. Paths authored in the file retain their configured project root when an inline root changes. Inline CSS and tsconfig paths use the resulting project root. Resolved paths are absolute. Analysis scope settings do not filter the bundler's
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

Compiler policy belongs to `stillwind.config.ts`. Each project accepts all lint rules, generated identifier prefixes, and `utilityOrder: "authored" | "tailwind"`. CSS supplies theme tokens, utilities, variants, and output slots.

```ts
import { defineConfig, lintPresets } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "src/theme.css",
      files: ["src/**/*.tsx"],
      lint: {
        extends: [lintPresets.recommended],
        rules: {
          "prefer-typed-relationships": "deny",
          "unknown-utility": "warn",
        },
      },
    },
  ],
});
```

Reporting-only rules accept `"warn"`, `"deny"`, or `"off"`. Rules with detection options also accept an object containing `severity` and their options. Omitted fields retain inherited settings or use the rule's default options. Source suppression comments control findings and fixes. Explicit sorting uses `sort` targets independently of the `sort-utilities` reporting level.

Node and browser hosts share project selection, path resolution, validation, and defaults. Configuration errors identify the project and invalid field. Compiler settings can be validated without constructing an analysis scope. `automatic` controls document CSS delivery through build integrations. Vite's `emission` remains a Vite-specific option.

## Semantic and source services

The Node Type Facts backend resolves import and export requests from TypeScript module
symbols. It uses the selected tsconfig's paths, custom conditions, types, and JSX settings.
The same service retains an incremental snapshot for subsequent declaration queries.
CSS has a separate resolver with the `style` condition.

The checker project inherits the selected tsconfig and retains its declaration files.
Prepared sources and framework globals become its other roots. The generated configuration clears inherited target include/exclude patterns.
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

Analysis workspaces capture configuration data, provider registries, and the project
list when opened. Reopen the workspace to apply new settings. Browser workspaces also
capture the saved file inventory; recreate them to apply changes to that inventory.
Document APIs update open buffers.

`EditorSnapshot` owns copies of source buffers, projection maps, dependencies,
fix actions, and sorting settings. Later graph edits or native session updates
do not change its answers. Node analysis graphs and sorting reports are
independent copies. Browser analysis graphs are readonly views shared by
cache reads; TypeScript callers can use `structuredClone(graph) as ModuleAnalysis`
to obtain editable records.
Editing these copies does not change cached queries. A project query selects the
snapshot for its current document revision.

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
`lintReports(results)` from `@stillwind/analysis` collects distinct reports per project without
discarding report fields or loading a compiler binding.
The CLI uses a smaller JSON projection described in its [README](../packages/cli/README.md);
the analysis APIs retain complete workspace results and graphs.

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
              return {
                status: "complete",
                reports: output.reports,
              };
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

## Lint presets

Import `lintPresets` from `stillwind/config` or `@stillwind/config`. Each project's
`lint` object contains `extends` and `rules`. Presets apply in array order, and the
project's rules apply last. A preset can extend other presets and supply an optional
`name` for configuration errors. Local modules and external packages can export the
same `LintPreset` shape.

```ts
import { defineConfig, lintPresets, type LintPreset } from "stillwind/config";

const team = {
  name: "team/design-system",
  extends: [lintPresets.recommended],
  rules: {
    "no-arbitrary-colors": "deny",
  },
} as const satisfies LintPreset;

export default defineConfig({
  projects: [
    {
      css: "src/theme.css",
      files: ["src/**/*.tsx"],
      lint: {
        extends: [team, lintPresets.stylistic],
        rules: {
          "sort-utilities": "off",
        },
      },
    },
  ],
});
```

Omitting `lint` selects `recommended`. An explicit `lint: false` or `lint: {}` disables every lint
rule. An explicit object with only `rules` enables only those rules. Compiler errors
are reported independently of lint settings.

`recommended` enables the standard correctness checks and canonical suggestions.
`strict` reports all recommended checks as errors and also checks unused lint
suppressions. `stylistic` enables `prefer-direct-styling`, `prefer-logical-properties`,
`prefer-typed-relationships`, and `sort-utilities` as warnings. The stylistic preset
adds no recommended checks. Project-specific restrictions remain off in these presets.

A severity-only override preserves inherited rule options. An option object changes
only supplied fields; undefined fields preserve previous values. Arrays replace the
whole inherited list. Off disables diagnostics and lint fixes while retaining options.
Cycles in `extends`, unknown fields, and invalid options fail configuration evaluation.
Required options are validated after composition.

Inline build overrides preserve the selected project's preset list when they omit
`extends`. A supplied list replaces it, including an empty list. Inline rules merge
with the project's rules. Without a configuration file, inline lint settings form the
project configuration and use the same omission and empty-object behavior.

An inline `lint: false` disables all rules from the selected project. An object override
of a project's `lint: false` enables only the override's rules and presets.

`resolveLintConfig` returns a `rules` object with explicit severities and default
options for every rule. Node and browser hosts use the same resolver, and native
bindings receive the resulting rule settings. Explicit editor sorting remains available
when `sort-utilities` is off.

## Utility lint policies

`duplicate-utilities` and `conflicting-utilities` default to `warn`. They compare
utilities supplied directly to one `style()` call, including its arrays and
variant wrappers. Comparisons require compatible source conditions and identical
expanded CSS selector and at-rule contexts. Composed bases and ref providers use
the existing composition rules. Identical declarations shared by separate utilities
do not conflict. Duplicate findings compare candidate spellings.

`conflicting-utilities.allowShorthandOverrides` defaults to `false`, so `p-2 px-4`
reports an overlap. Setting it to `true` allows a narrower declaration to override
part of a shorthand. Two different values for the same property still conflict.
Logical and physical longhands still conflict when their relationship depends on
writing mode.

The other utility policies default to off. Supplying a policy object enables it at `warn`
unless it inherits a severity from a preset or specifies one in that object.

```ts
lint: {
  extends: [lintPresets.recommended],
  rules: {
    "prefer-direct-styling": "warn",
    "prefer-logical-properties": "warn",
    "length-units": {
      rules: [
        {
          properties: ["*"],
          disallow: ["px"],
        },
        {
          properties: ["border-*-width", "outline-width"],
          disallow: ["rem"],
        },
      ],
    },
    "no-restricted-theme-tokens": {
      tokens: ["--color-red-*", "--spacing-ref-*", "--radius-ref-*"],
    },
    "no-arbitrary-breakpoints": "warn",
  },
}
```

The arbitrary-value, class, utility, declaration, and important restrictions default
to off. Object options enable a rule at `warn` unless a preset or the object supplies its
severity.

```ts
lint: {
  extends: [lintPresets.recommended],
  rules: {
    "no-arbitrary-colors": "warn",
    "no-arbitrary-spacing": {
      properties: ["width", "inset", "inset-*"],
    },
    "no-class-property": "warn",
    "no-important": "warn",
    "no-restricted-utilities": {
      utilities: [
        {
          pattern: "legacy-*",
          message: "Use semantic utilities",
        },
      ],
    },
    "no-restricted-declarations": {
      rules: [
        {
          properties: ["transition-property"],
          values: ["all"],
        },
      ],
    },
    "valid-lint-suppressions": "warn",
    "no-unused-lint-suppressions": "warn",
  },
}
```

`no-arbitrary-colors` checks explicitly supplied values before theme expansion.
It reports `bg-[#fff]`, `[color:#fff]`, and literal colors within gradients,
borders, and shadows. Named utility definitions and theme values may contain
literal colors. Variable references, `transparent`, `currentColor`, and CSS-wide
keywords are allowed. Strings and URL arguments are skipped. Variable fallbacks
are checked. Color functions with variable-dependent channels are allowed;
literal colors in those functions remain subject to inspection.

`no-arbitrary-spacing` checks nonzero lengths and percentages in arbitrary margin,
padding, and gap values, including values inside math functions and variable
fallbacks. Zero values, keywords, variable references, and named spacing values
are allowed. `properties` adds case-insensitive complete property globs using
`*` and `?`. A matching shorthand also selects its components. Width, height,
inset, and border width are excluded unless added.
Fixed declarations inside named utility definitions are allowed.

Both arbitrary-value rules also inspect literal custom-property assignments when
a utility in the same call consumes them in the relevant property domain. They
follow local literal aliases with a cycle and depth bound. An unused variable,
a runtime value, a conditional assignment, a competing assignment, or a call
with composed props cannot establish a definite literal value and is skipped.
Diagnostics highlight the literal assignment when it supplies the forbidden value.

`no-class-property` reports explicit `class` contributions inside style objects,
including contributions inside arrays and JavaScript conditions. It allows
composition of completed styles and does not inspect generated `class` or
`className` output. External classes are not inspected by utility restrictions.
An ordinary `style` key is already an invalid macro input; generated inline style
remains available for custom properties and dynamic view transitions.

`no-restricted-utilities.utilities` contains complete, case-sensitive utility globs
using `*` and `?`, with optional diagnostic messages. Matching removes enclosing
variants and important modifiers but keeps signs and slash modifiers. Unresolved
utility names are checked too. Canonical suggestions cannot introduce a restricted
utility. The rule does not rewrite a forbidden name automatically.

`no-restricted-declarations.rules` matches emitted declarations and parsed shorthand
components, including declarations inside named utilities and nested rules.
Property and serialized value globs ignore ASCII case and match the complete text.
The priority suffix is removed before value matching. Every matching restriction
applies, and an optional `message` explains the project's policy. Values remain CSS
text, so patterns can also match quoted strings and URL arguments.

`no-important` reports important declarations from explicit modifiers, arbitrary
properties, and named utility bodies. These declaration policies share the utility
emitter's resolved declarations and do not inspect unrelated global stylesheet rules.

`valid-lint-suppressions` defaults to `warn`. It reports unknown rule names and
malformed `swlint-*` keywords at their source locations. Explanations after `--`
are allowed. Unknown names never select other rules. A directive cannot suppress
a validation finding about itself; other directives can suppress this rule.

`no-unused-lint-suppressions` defaults to off. It reports each unused named rule in a disable comment,
or one finding for an unused wildcard disable. Only enabled rules and rules with
reported findings participate. Suppression validation rules do not count as uses.
Sorting is performed separately and does not participate in this check. Failed
analysis skips unused checks. Project sessions combine source findings with current
composition findings before checking usage again when a base changes. A disable
comment cannot hide its own unused finding. A named enable restores that rule even
under a wildcard disable.

`prefer-direct-styling` encourages colocating styles on the element they affect.
It reports variants that select another element, including mixed selector lists.
Pseudo-elements are classified by their originating element: `&::before` and
`&::part(...)` are allowed, while `& > span::before` reports the indirect selection
of `span`. Ancestor conditions such as `.parent &` and `&:has(...)` keep the
attachment target. Internal selectors authored in utility definitions are allowed.
Selectors the compiler cannot classify are skipped.

`prefer-logical-properties` checks emitted declarations with logical counterparts,
including `text-align: left`, `float: right`, and asymmetric four-side shorthands.
Uniform four-side shorthands are allowed. The rule does not rewrite declarations
because their meaning depends on writing mode.

`length-units` uses complete property-name globs with `*` and `?`, ignoring ASCII
case. The last matching rule replaces earlier restrictions. Parsed shorthands use
their component properties, which can override a restriction on the shorthand.
Zero lengths, strings, and URLs are excluded. Known theme values are followed with
a cycle and depth bound; unresolved runtime variables are skipped. Restrictions
can prohibit `rem` as well as `px`.

`no-restricted-theme-tokens.tokens` must contain at least one complete custom-property
name pattern starting with `--` when the rule is enabled. Patterns use case-sensitive
`*` and `?` globs. The rule covers defined theme tokens of every type. The compiler
records direct selections by utilities and variants before inline expansion.
A semantic token may refer to a restricted token internally. Its own name is checked
when used directly. Unresolved runtime variables are excluded. The compiler does not
infer token roles from values or alias relationships.

`no-arbitrary-breakpoints` reports viewport width and height comparisons supplied
directly in variants, such as `max-[900px]`, `min-[48rem]`, and
`[@media(width>=900px)]`. Range expressions, legacy min/max features, and compound
media conditions are included. A value equal to a named breakpoint still reports.
Named theme captures and fixed conditions in named custom variants are allowed.
Custom variants that accept explicit size values are checked through the selected
value domain. Other media features, support conditions, container queries, and media
queries inside utility definitions are excluded. One wrapper produces one finding
per offending variant, even when it contains several utility literals.

These policies provide diagnostics without automatic fixes. Canonical utility
suggestions are omitted when their replacement violates an enabled policy. Source
suppression uses the same rule names, for example
`// swlint-disable-next-line duplicate-utilities`.

## Editor sorting

`sort.arrays` groups direct arrays, `style()` argument lists, and object entries by input kind. It defaults to true. Ref groups retain their first appearance order, with each ref's marker before its projections. Bases, utilities, custom properties, and attributes follow. Entries in each category retain their original order. A base can contain variants, custom properties, or composed styles; the sorter uses its identity without inspecting its body or CSS.

`sort.stringLiterals` defaults to true and sorts utility tokens inside each static string or template segment using compiler order. This does not reorder separate utility array elements. Unknown utility tokens delimit sortable string runs.

`sort: false` disables utility sorting, input grouping, and attribute name sorting.
It also disables sorting diagnostics and sorting fixes. Other lint rules and fixes
remain active. An inline `sort: false` replaces the project's sort options; an object
override of false uses the default targets for omitted fields.

`sort.attributes` defaults to false. Set it to true to sort adjacent `aria-` and `data-` property names. It works independently of the other two options. Duplicate names keep their original order. Spreads, unsupported expressions, ambiguous comments, and unmapped source regions delimit sortable input runs. Explicit sorting is available when sorting diagnostics are off; lint fixes respect source suppression.

```ts
import { defineConfig } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "theme.css",
      files: ["src/**/*.tsx"],
      sort: {
        arrays: true,
        stringLiterals: true,
        attributes: true,
      },
    },
  ],
});
```
