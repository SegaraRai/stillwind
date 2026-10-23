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
The [integration guide](integrations.md) describes plugin and wrapper registration
for each build tool. Each entrypoint accepts configuration discovery and inline
override options. Pass those options as the second argument to Next's `withStillwind`.

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
relative to the application root. Paths authored in the file retain their configured
project root when an inline root changes. Inline CSS and tsconfig paths use the
resulting project root. Resolved paths are absolute. Analysis scope settings do not
filter the bundler's module graph. Build-specific settings such as Vite CSS emission
stay in the integration configuration.

`@stillwind/config` owns the shared definitions, export evaluation, validation, and
merge operation. `stillwind/config` re-exports that public API. `@stillwind/config/node`
owns discovery, trusted JS/TS evaluation, project selection, and path resolution.
Browser consumers can resolve an in-memory export through the shared entrypoint.

Vite restarts its server when a configuration dependency changes. Production watch
builds and webpack/Rspack compilations evaluate settings again before compiling.
Next and Turbopack pass configuration references to loaders; configuration callbacks
stay in the file rather than being serialized into loader options. Inline naming
callbacks remain unsupported by these loaders.

Compiler policy belongs to `stillwind.config.ts`. Each project accepts all lint rules,
generated identifier prefixes, and `utilityOrder: "authored" | "tailwind"`. CSS supplies
theme tokens, utilities, variants, and output slots.

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

Reporting-only rules accept `"warn"`, `"deny"`, or `"off"`. Rules with detection options
also accept an object containing `severity` and their options. Omitted fields retain
inherited settings or use the rule's default options. Source suppression comments
control findings and fixes. Explicit sorting uses `sort` targets independently of the
`prefer-sorted-utilities` reporting level.

Node and browser hosts share project selection, path resolution, validation, and
defaults. Configuration errors identify the project and invalid field. Compiler settings
can be validated without constructing an analysis scope. `automatic` controls document
CSS delivery through build integrations. Vite's `emission` remains a Vite-specific option.

## Semantic and source services

The Node Type Facts backend resolves import and export requests from TypeScript module
symbols. It uses the selected tsconfig's paths, custom conditions, types, and JSX settings.
The same service retains an incremental snapshot for subsequent declaration queries.
CSS has a separate resolver with the `style` condition.

The checker project inherits the selected tsconfig and retains its declaration files.
Prepared sources and framework globals become its other roots. The generated
configuration clears inherited target include/exclude patterns.
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

## Analysis lifetimes

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

Each target's cross-module analysis follows the semantic imports needed by its
type probes; results report `coverage: "observed"`. CLI discovery, file selection,
rewrite guarantees, exit codes, and output formats are documented in the
[CLI reference](cli.md).

## Editor behavior

The language server discovers configuration from each workspace folder.
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

## Editor previews

Hovers on utilities, `style()` calls, and bound variables show generated CSS with
the main selector written as `&`. Completion uses the active design system and
resolves selected items to CSS and declaration or variant summaries. Variant
suggestions omit variants already present, continue completion after insertion,
and offer editable arbitrary values when the client supports snippets.

The project's `preview` field controls comments added after preview values.
`themeValues` adds resolved theme values and evaluated `calc()` results.
`pixelEquivalents` adds pixel values for rem. `colorEquivalents` adds hexadecimal
sRGB colors. All three are enabled by default.

`colorEquivalents: "approximate"`, the default, maps colors outside sRGB into the
gamut and marks them with `≈`. `"exact"` omits out-of-gamut colors; `false` omits all
color comments. Numbers are rounded to six fractional digits, and `≈` also marks
rounded numbers, such as `calc(1.25 / 0.875) ≈ 1.428571`.

A `calc()` that evaluates to one value receives one comment after the expression.
An expression combining rem and px receives a pixel total when `pixelEquivalents`
is enabled, such as `calc(var(--spacing) + 1px) /* 5px */`. Other expressions receive
comments on theme references inside them.

`rootFontSize`, which defaults to 16, controls pixel equivalents and canonical
px-to-rem utility suggestions. Media and container query comments always use 16,
because browsers evaluate rem there against the initial font size.

Quoted `@plugin` paths complete configured plugin names, package dependencies,
explicit export subpaths, local scripts, and directories. Resolved paths link to
module files; configured plugins without a module file have no link. These queries
use captured files, including unsaved buffers, without executing plugins or
requiring a valid CSS graph.

## Stylesheet lint

Stylesheet lint checks the project's CSS entry and the CSS files that it imports by a
relative or absolute path. A file under `node_modules`, a file imported by a package
specifier such as `stillwind`, and every file that such a file imports are package
CSS, and lint skips them. `loadCssGraph` marks each file when it supplies the file to
the native loader. A host can replace this choice with `CssHost.lint`. Lint never
changes the compiled CSS.

The rules check `@apply` candidates and `@variant` sequences inside style rules:
`unknown-utility`, `unknown-variant`, `invalid-utility`, `invalid-variant`,
`invalid-marker-use`, `incompatible-property-value`, `clamped-value`,
`redundant-variant`, `unsatisfiable-variants`, `prefer-typed-variables`,
`prefer-canonical-utilities`, `prefer-sorted-utilities`, `document-scoped-selector`, and
`repeated-nesting-selector`.
Each finding covers the candidate or the part of it that causes the finding, and fixes
edit the stylesheet. The selector of the enclosing rule is the anchor of a candidate,
as [Variant targets](macro-contract.md#variant-targets) describes. `duplicate-utility`
and `conflicting-utilities` compare the candidates of one `@apply` statement.
`redundant-variant` and `unsatisfiable-variants` check a `@variant` sequence or an
`@apply` candidate together with the sequences of the enclosing `@variant` blocks. A
nested style rule starts a new stack, because it selects another element.

Other rules check only stylesheets. They check definitions and theme references:

- `prefer-explicit-nesting-selector` (style, default `warn`) reports a selector of a
  `@custom-variant`, `@variant`, or `@stillwind-variant` definition that has a branch
  without `&` or a branch that starts with a combinator, such as `(:hover)`,
  `(> .item)`, or `{ :hover { @slot; } }`. Such a selector is relative to `&` in every
  context, so `(:hover)` means `(& :hover)`. Definitions with `scope(document)` are not
  reported. The finding covers the selector at the definition and never covers a use.
  Its fix inserts `& ` before each such branch, which keeps the meaning. The message
  names `scope(document)` for a selector that should select a document-level element,
  but no fix adds it, because the marker changes the meaning.
- `bypassed-undefinition` (hazard, default `warn`) reports a `@utility`,
  `@custom-variant`, or `@variant` definition outside every `@stillwind-registry` block
  that registers after an undefinition of the `keyframe` or `view-transition` registry
  with a matching pattern. Such a definition reaches that registry although the
  undefinition removed the earlier definitions of its name. A later undefinition of the
  registry, or an undefinition that also changes `default`, is not bypassed. The
  finding covers the definition name.
- `duplicate-definition` (hazard, default `warn`) reports an earlier `@custom-variant`
  or `@utility` that later definitions with the same name hide completely, so it is
  never used. A later variant definition hides an earlier one when it resolves.
  Later `@utility` definitions hide an earlier one when they set every property of
  its declarations, because exact utility definitions merge. Definitions with a
  capture, such as `tab-*`, are overloads, and redefinitions of the package's names
  such as `dark` are customizations; neither is reported. The finding covers the name
  of the hidden definition, and a related location covers the later definition.
  Definitions are ordered after import expansion, so the later definition can be in
  another file.
- `incompatible-theme-value` (hazard, default `deny`) reports a `@theme` token value that
  matches no value type of its namespace, such as `--color-brand: 12px` or
  `--breakpoint-tab: blue`. The value types come from `@stillwind-theme-namespace`:
  `<color>` for `--color-*`, `<length>` for `--spacing`, `--breakpoint-*`, and
  `--container-*`, a font size for `--text-*`, and so on. A shadow namespace accepts a
  comma-separated list of shadows. Values that contain `var()`, `theme()`, or
  `--theme()` are not checked. The output does not change.
- `ignored-tailwind-feature` (hazard, default `warn`) reports Tailwind features that
  loading ignores. The loader removes a top-level `@config` statement, whose
  replacement is the project's `tailwind` field. It loads an import without the
  `important`, `prefix`, `reference`, `source`, and `theme` modifiers. The project's
  `important` field replaces the `important` modifier. The finding covers the
  statement or the modifier.
- `invalid-css-import` (input, default `deny`) reports an import that loading skipped:
  an `@import` or `@plugin` statement with an invalid specifier, options, or modifiers,
  an import that closes a cycle, and an import that the host cannot resolve or read.
  The finding covers the statement.
- `invalid-css-directive` (input, default `deny`) reports a Stillwind directive that
  loading skipped. [Skipped statements](css-emission.md#skipped-statements) lists them.
  The finding covers the directive.
- `ignored-definition-condition` (hazard, default `deny`) reports a definition inside a
  conditional at-rule, such as `@utility` inside `@media`. Definitions apply at compile
  time, so loading keeps the definition and ignores the condition. Put the condition
  around `@stillwind-slot utilities;` instead. The finding covers the definition.
- `invalid-runtime-condition` (input, default `deny`) reports a declaration whose
  `--if()` cannot be lowered with the selected `useCssIf` mode. The stylesheet output
  leaves the declaration out. The finding covers the declaration.
- `invalid-theme-reference` (input, default `deny`) reports a `theme()` or `--theme()`
  call that names an inactive token without a fallback, in authored rules, `@utility`
  bodies, and variant definitions. Stylesheet rendering omits the declaration that
  contains such a `--theme()` call, or the rule whose selector or prelude contains it.
  A `--theme()` fallback of `initial` omits the declaration on purpose and is not
  reported. The finding suggests an active token with a close spelling. The rule also
  reports, at the name, a `@utility` with a capture whose `--value()` domains are
  only theme namespaces that provide no value. A namespace can have any name, such as
  `--size-*`, and it provides no value when it has no active token with the pattern's
  suffix. Uses of that utility still report `unknown-utility`.

`prefer-variant-directive` (style, default `off`, enabled by the `stylistic` preset)
also checks only stylesheets. It reports a nested style rule or a conditional at-rule
inside a style rule when `@variant <name>` expands to exactly the same header. Its fix
replaces only the header and keeps the block body:

| Authored CSS                                  | Fix                      |
| --------------------------------------------- | ------------------------ |
| `@media (width >= 40rem) { ... }`             | `@variant sm { ... }`    |
| `&:focus-visible { ... }`                     | `@variant focus-visible` |
| `@media (hover: hover) { &:hover { ... } }`   | `@variant hover { ... }` |
| `@media (prefers-color-scheme: dark) { ... }` | `@variant dark { ... }`  |

The rule expands each variant spelling that completion offers and indexes the
spellings whose expansion is one chain of headers. A two-level variant such as `hover`
matches only when both levels are written, in either order, and its fix replaces both
headers. When several spellings expand to the same headers, such as `sm` and `min-sm`,
the fix uses the shortest one. The output selectors, and therefore their specificity,
do not change.

The comparison ignores whitespace and comments, ASCII case in `@media` conditions and
pseudo-class names, and spacing in `@supports` conditions. It reads `(min-width: 40rem)`
as `(width >= 40rem)` and `(max-width: 40rem)` as `(width <= 40rem)`. The package's
`max-*` variants use `width <`, so `max-width` never matches them.

The rule does not report these headers:

- `&:hover` alone, because the package's `hover` also applies `@media (hover: hover)`.
- Selector lists such as `&:hover, &:focus`, and variants such as `not-hover` that
  emit several rules or add declarations.
- At-rules outside style rules, because `@variant` there defines a variant.
- Headers inside `@custom-variant`, `@stillwind-variant`, and `@utility` bodies, where
  the variant could refer to itself, and headers in files that are not linted.

The option `pxToRem` (default `false`) lets a `px` length in a `@media` condition
match a `rem` length of a variant at 16px = 1rem, so `(min-width: 640px)` becomes
`@variant sm`. This option is separate from `prefer-canonical-utilities.pxToRem`. A
`rem` length in a media query follows the browser's default font size, so after the
fix the breakpoint switches at a different width for users who changed that size. The
message of such a finding says so.

The rule catalog records whether each rule checks scripts, stylesheets, or both. Macro
rules, such as `mutating-macro-argument` and the ref and view-transition rules, never
check stylesheets. One severity setting applies to every source that a rule checks.
In a stylesheet, only `/* ... */` comments carry `swlint-*` directives. A directive
that names a rule that does not check stylesheets reports `invalid-lint-suppression`.

`stillwind lint`, `lint --fix`, and `fmt` add each project's CSS entry and imported user
stylesheets to the script targets. Explicit file patterns filter both kinds of targets.
The language server publishes
diagnostics and quick fixes for open CSS documents and uses unsaved CSS buffers when
it loads the design system. When the CSS graph cannot load, script analysis reports
the failure, and no stylesheet is linted.

CSS editor queries complete Stillwind at-rule names, `@apply` candidates, applied
`@variant` sequences, output-slot arguments, and active theme names in `theme()`,
`--theme()`, and `var()`. Hovers show generated candidate CSS, variant wrappers,
directive and slot documentation, or the active theme definition. A `var()` hover
describes that definition; the browser's cascade can override its value.

At-rule discovery separates **Authoring**, **Registry**, and **Compiler** audiences.
Ordinary completion offers authoring syntax, including Stillwind directives for output
slots, CSS templates, custom-property registrations and requirements, and
`@stillwind-registry` blocks and undefinitions. Registry syntax describes utility
libraries and resolution metadata. Compiler intrinsics appear only in the direct body of
a registry variant definition. Known blocks narrow the suggestions: variant definitions
offer `@slot`, and style rules offer `@apply` and applied `@variant`. A fragment without
an enclosing block retains broad authoring suggestions and ranks names for stylesheet
placement first.

VS Code's `stillwind.completion.registryDirectives` setting adds registry suggestions; it
defaults to false. Other LSP clients can send `initializationOptions.completion.registryDirectives`
and update it with `workspace/didChangeConfiguration` under
`settings.stillwind.completion.registryDirectives`. Direct editor requests pass
`registryDirectives` with both completion and resolution. This editor setting does not
change compiler output or lint. Changing it invalidates earlier CSS completion resolutions.
Written directives retain hover and supported argument completions even when their names
are hidden. Property requirements use registered property names; format arguments use
active registry metadata; intrinsic arguments use the compiler's supported identifiers.

Completion details and hovers show the audience and the independent syntax family,
**Tailwind-derived** or **Stillwind**. The family describes provenance and does not
guarantee Tailwind compatibility. Lower-level syntax is not deprecated by this classification.

Color queries resolve `@apply` candidates and complete `theme()`, `--theme()`, and
`var()` references to active theme values. Unknown, cyclic, contextual, and ambiguous
colors are omitted. References inside `@apply` candidates are handled through the
whole candidate. A color presentation preserves a utility's variants, importance, and
modifier, and returns no edit when the candidate cannot reproduce the requested color.
Changing a reference replaces that entire function call with a color literal at the
current use. It does not edit the referenced theme declaration. Ordinary CSS color
literals remain available through the editor's standard CSS provider.

Queries accept incomplete blocks and candidates. When original CSS cannot load,
the editor retries with recovered imports and definitions. This recovery does not
supply compilation output, lint diagnostics, sorting, or lint fixes. Color queries
and color presentations can use recovered definitions. Unreadable files and
unresolved imports can still prevent semantic queries. Completion resolution
requires the same document, design-system revision, and preview settings.

Build, development, and watch sessions of bundler integrations skip stylesheet lint,
as they skip module lint, so lint never changes whether a build succeeds or what it
emits. Hosts that load CSS themselves call `NativeDesignSystemLoader.stylesheetReports()`
or `NativeProjectSession.stylesheetReports()`, which use the lint settings of the
loader or session. The analysis runs once for each design-system revision and set of
options.

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
  onExit: (shutdown) => {
    process.exit(shutdown ? 0 : 1);
  },
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
  onExit: () => {
    self.close();
  },
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
          "prefer-sorted-utilities": "off",
        },
      },
    },
  ],
});
```

Omitting `lint` selects `recommended`. An explicit `lint: false` or `lint: {}` disables
every lint rule. An explicit object with only `rules` enables only those rules. Compiler
errors are reported independently of lint settings.

Each rule has a category in `lintRules`: `input` reports input that lowers as empty or
false, `contract` reports a violation of the macro contract, `hazard` reports valid output
that is probably unintended, `restriction` reports a project-specific prohibition, `style`
reports a preferred spelling, and `suppression` checks `swlint-*` directives.

`recommended` enables every `input`, `contract`, and `hazard` rule, invalid suppression
directives, canonical suggestions, `redundant-variant`, and
`prefer-explicit-nesting-selector`. `strict` reports all
recommended checks as errors and also checks unused lint suppressions. `stylistic`
enables `prefer-direct-styling`, `prefer-logical-properties`,
`prefer-typed-relationships`, `prefer-sorted-utilities`, `prefer-typed-variables`, and
`prefer-variant-directive` as warnings. The stylistic preset adds no recommended checks.
Project-specific restrictions remain off in these presets.

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
when `prefer-sorted-utilities` is off.

## Utility lint policies

`duplicate-utility` and `conflicting-utilities` default to `warn`. They compare
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

`invalid-marker-use` defaults to `warn`. Its `ignoredClasses` option lists exact
decoded marker class names, such as `group` or `foo/bar`, whose findings are not
reported. A supplied list replaces the inherited list.

The other utility policies default to off. Supplying a policy object enables it at
`warn` unless it inherits a severity from a preset or specifies one in that object.

```ts
lint: {
  extends: [lintPresets.recommended],
  rules: {
    "prefer-direct-styling": "warn",
    "prefer-logical-properties": "warn",
    "no-restricted-length-units": {
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
to off. Object options enable a rule at `warn` unless a preset or the object supplies
its severity.

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
    "invalid-lint-suppression": "warn",
    "unused-lint-suppression": "warn",
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

`invalid-lint-suppression` defaults to `warn`. It reports unknown rule names and
malformed `swlint-*` keywords at their source locations. Explanations after `--`
are allowed. Unknown names never select other rules. A directive cannot suppress
a validation finding about itself; other directives can suppress this rule.

`unused-lint-suppression` defaults to off. It reports each unused named rule in a disable
comment, or one finding for an unused wildcard disable. Only enabled rules and rules with
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

`prefer-logical-properties` and `no-restricted-length-units` accept
`checkImplementation`, which defaults to `false`. Fixed declarations from an
`@utility name (implementation) { ... }` definition are excluded by default.
Set this option to `true` on either rule to include them. Declarations that consume
candidate values through placeholders remain checked, as do unmarked definitions
that extend the same utility.

`no-important`, `no-restricted-declarations`, and `no-restricted-theme-tokens` also
accept `checkImplementation`, with a default of `true`. Set it to `false` to exclude
fixed implementation declarations from that rule. Source important modifiers,
declarations that consume candidate values, and unmarked extensions remain checked.
The token rule also checks tokens selected through variant arguments.

For custom properties that carry values for ordinary properties, `@stillwind-property`
accepts `value-for: width, height;`. The arbitrary color, arbitrary spacing, and length
unit policies check each listed property. For example, Iconify declares
`value-for: height;` for its size property, so restrictions configured for `height`
also check `icon-[20px]`. Sizing properties such as `height` must still be included in
`no-arbitrary-spacing.properties` to extend that rule's default spacing properties.
See [Utility bodies](css-emission.md#utility-bodies) for the descriptor's contract.

Built-in value properties use their receiving properties for these policies, including
`translate`, `border-spacing`, `filter`, `backdrop-filter`, `background-image`,
`mask-image`, `box-shadow`, `text-shadow`, `scrollbar-color`, `letter-spacing`, and
`line-height`. Alpha and reverse flags keep their own classifications. Arbitrary color
inspection includes colors inside mask gradients and filter functions.
Utilities in `style.keyframes()` definitions use these value policies in the
`keyframe` registry context.

```ts
lint: {
  rules: {
    "no-important": { severity: "warn", checkImplementation: false },
    "no-restricted-declarations": {
      checkImplementation: false,
      rules: [{ properties: ["position"], values: ["fixed"] }],
    },
    "no-restricted-theme-tokens": {
      checkImplementation: false,
      tokens: ["--color-ref-*"],
    },
  },
}
```

`no-restricted-length-units` uses complete property-name globs with `*` and `?`, ignoring
ASCII case. The last matching rule replaces earlier restrictions. Parsed shorthands use
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
`// swlint-disable-next-line duplicate-utility`.

## Editor sorting

`sort.arrays` groups direct arrays, `style()` argument lists, and object entries by
input kind. It defaults to true. Ref groups retain their first appearance order, with
each ref's marker before its projections. Bases, utilities, and custom properties
follow. Entries in each category retain their original order. A base can
contain variants, custom properties, or composed styles; the sorter uses its identity
without inspecting its body or CSS.

`sort.stringLiterals` defaults to true and sorts utility tokens inside each static
string or template segment using compiler order. This does not reorder separate utility
array elements. Unknown utility tokens delimit sortable string runs. A token that adds a
[marker class](css-emission.md#marker-classes) and resolves no utility sorts before the
utilities of its run, as Tailwind sorts classes without CSS first. Such tokens keep their
relative order.

`sort: false` disables all sorting targets in scripts and stylesheets.
It also disables sorting diagnostics and sorting fixes. Other lint rules and fixes
remain active. An inline `sort: false` replaces the project's sort options; an object
override of false uses the default targets for omitted fields.

Spreads, unsupported expressions, ambiguous comments, and unmapped source regions
delimit sortable input runs.

The CSS targets are independent:

- `sort.applyUtilities` defaults to true. It sorts utility arguments inside each complete
  `@apply` statement using compiler order. Unknown utilities and comments delimit sortable runs.
  A marker class name that resolves no utility sorts first, as in scripts.
- `sort.applyStatements` defaults to false. It orders adjacent `@apply` statements by their
  utilities' compiler order. It keeps each statement intact and leaves argument sorting to
  `applyUtilities`.
- `sort.variantBlocks` defaults to false. It orders adjacent applied `@variant` blocks by
  compiler variant order. It preserves each header and never moves variant definitions.

Whole statements and blocks move only among siblings of the same kind separated by whitespace.
Competing property writes retain their relative order, even under different variants. This
includes shorthand and longhand properties, logical properties, `all`, and custom properties.
Comments, ordinary declarations and rules, unknown effects, and protected ranges stop movement.
The sorter combines inner edits with outer movement and returns nonoverlapping edits in original
source coordinates. Explicit sorting remains available with `lint: false` or sorting diagnostics
off. Lint fixes respect source suppression and use the same CSS edit planner.

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
        applyUtilities: true,
        applyStatements: true,
        variantBlocks: true,
      },
    },
  ],
});
```
