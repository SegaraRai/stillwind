# Open work

This file contains unresolved design and validation work. Current supported
inputs and failure behavior are documented with their APIs. Each item below
identifies a remaining limitation and the evidence needed to resolve it.

## Conditional utility completions

Utility strings in `style()` conjunctions, such as `style(enabled && "opacity-50")`,
still suggest their own inferred strings. Evaluate suppression while preserving ref
template inputs and finite macro choices.

Sources: [style declaration](../packages/core/src/macro/style.ts),
[input checks](../packages/core/src/macro/input.ts).

## Playground editor state

The Playground reports `Cannot set null/undefined state` browser errors during static
acceptance. Trace the Monaco state update that receives the missing value. Confirm that
formatting, completion, and HMR complete with an empty browser error list.

Source: [static acceptance](../e2e/play/src/static.test.ts).

## Semantic fingerprints and prefix rewriting

Semantic revisions hash `Debug` representations of facts and asset metadata in
several entrypoints. Define one deterministic representation shared by those
entrypoints. Confirm invalidation when facts or asset support change without
making incidental debug formatting part of the cache contract.

Sources: [project input](../crates/napi/src/api/project.rs),
[session facts](../crates/napi/src/session/facts.rs),
[compiler API](../crates/compiler/src/compiler/api.rs).

`DesignSystemOptions::rewrite_generated` and `CompilerOptions::rewrite_generated`
duplicate literal prefix substitution. Consolidate the responsibility and define
how generated identifiers stay separate from authored strings. Preserve swapped
and overlapping configured prefixes and resource references.

Sources: [design-system options](../crates/design-system/src/options.rs),
[compiler options](../crates/compiler/src/options.rs).

## Selective CSS reloads

Definition dependencies remain conservative, and successful CSS reloads recompile
retained analyses. Selective invalidation needs dependencies for successful
resolution, failed lookup, new definitions, and namespace resets. Verify added,
changed, and removed definitions before using narrower reload selections.

Sources: [module compiler](../crates/compiler/src/module_compiler.rs),
[project session](../crates/project/src/session/state.rs).

## Assets from compiled packages

Assets whose declaration source is absent from the graph have no supported
versioned package manifest channel. Define identity records, value-shape versions,
naming revisions, CSS ownership, and collision reservations before accepting them.
Keep checker propagation separate from the delivery of actual asset definitions.

Sources: [module graph](../packages/host/src/compilation/module-graph.ts),
[semantic values](../crates/semantic/src/symbolic.rs).

## View-transition keys

The linter counts transition-name suppliers but does not detect repeated-render
or unstable-key use. Establish which framework facts can support actionable
warnings without treating valid cross-component pairing as a collision.

Source: [view-transition analysis](../crates/compiler/src/style_analysis/view_transition.rs).

## Public documentation references

Keep `/docs/`, `/docs/frameworks/`, and `/docs/style/` as the entry guides. Use the
following route structure when adding the references and task guides:

| Route                       | Content                                                             |
| --------------------------- | ------------------------------------------------------------------- |
| `/docs/guides/<task>/`      | Task guides with complete examples and links to API contracts       |
| `/docs/utilities/`          | One searchable catalog with utility-to-CSS and CSS-to-utility modes |
| `/docs/macros/<macro>/`     | Macro inputs, consumption sites, runtime behavior, and emitted CSS  |
| `/docs/configuration/lint/` | Presets, severity, rule options, suppression, and build behavior    |
| `/docs/lints/<rule-id>/`    | One reference per public kebab-case lint ID                         |
| `/docs/errors/<SW-code>/`   | One reference per explicit compiler error code, preserving case     |

Use `style`, `ref`, `keyframes`, `view-transition`, `anchor-scope`, `media`, `css`,
and `cva` for macro slugs. Give related methods explicit anchors on their owning
page. Guide slugs should describe tasks: `element-relationships`,
`responsive-styles`, `animations`, `view-transitions`, `reusable-styles`,
`design-systems`, and `lint-adoption`. Guides explain complete workflows; macro
pages state the API contract and link back to those workflows.

Store published slugs and anchors explicitly. Keep redirects or aliases after a
rename, and retain a retirement page when a diagnostic disappears. Compiler codes
can currently change before the first release. Freeze published code identities
or record their aliases before exposing permanent diagnostic links from tooling.

The utility explorer should preserve its mode and query in the URL, for example
`/docs/utilities/?mode=utility&q=p-4` or
`/docs/utilities/?mode=css&q=display%3Aflex`. Build the default-theme catalog from
the registry, and resolve numeric and arbitrary candidates on demand. These
domains are not finite. Distinguish utilities from marker classes that emit no
CSS. Show the candidate's CSS and any support declarations separately.

Forward lookup can reuse `completeCandidate()` and `previewCandidate()`. The
latter omits document support declarations, so complete compilation examples must
also use the compiler. Start CSS lookup with property and value searches over
these generated records. Label partial matches and show the complete utility
output. Arbitrary CSS equivalence has no public query API. If exact reverse
conversion is added, reuse canonical normalization and required-property checks
in Rust, and define its supported input shape. Preserve selectors, conditions,
declaration order, and support requirements. Show the compiler version and theme
that determine each result.

Generate macro examples from actual compilations with their framework and
compiler options. Explain the general runtime form, then show the actual
expression, fold, and render stages and the selected output: direct props, a
class expression, or a runtime merge. Cover static input, conditions, custom
properties, composition, and finite declaration selection. The compiler chooses
between these forms; it has no public switch that exposes every stage as a
separate optimization level. A step-by-step display needs compiler trace data.
Keep bundler minification separate from compiler lowering.

Generate lint IDs, categories, defaults, options, and compiler error codes from
their existing catalogs. Add curated examples that verify the reported code,
location, effective settings, and corrected input. Describe whether an input is
omitted or output is kept, independently of lint severity. Configuration and
authoring errors need reproducible failing and passing inputs. Host and internal
errors need the violated contract, reproduction steps, and recovery or reporting
guidance when application source cannot fix them.

Feed the explorer, HTML references, per-page Markdown, and agent indexes from
the same generated records. Include the static utility catalog and compilation
examples in Markdown so agent readers receive the reference without browser
interaction. Check registry coverage, preserved URLs, diagnostic examples, and
generated output. Validate the explorer's two modes in the browser.

Sources: [query API](../packages/native/binding.d.ts),
[canonical index](../crates/compiler/src/query/canonical/index.rs),
[macro API](../packages/core/src/macro/style.ts),
[fragment compilation](../crates/compiler/src/compiler/fragment/mod.rs),
[runtime forms](runtime.md#lowering-and-materialization),
[lint catalog](../crates/diagnostics/src/lint/config/catalog.rs),
[generated lint metadata](../packages/config/src/lint-rules.ts),
[compiler error codes](../crates/diagnostics/src/compilation/code.rs),
[website compilation examples](../apps/website/src/lib/examples.ts).

## Measurement coverage

`TypeScriptFactsService` enables checker timing collection but exposes only
process, snapshot, and filesystem counters. Expose round trips, transferred bytes,
and server time in host debug telemetry so checker work can be distinguished
from transport and host work.

Source: [checker service](../packages/host/src/semantics/type-facts.ts).

Synthetic build/watch benchmarks and reused checker workloads do not establish
latency for a typed application through filesystem events, subsequent module
requests, browser CSS application, and framework HMR. Define and measure that
complete path before assigning application latency guarantees.

Sources: [architecture benchmarks](../benchmarks/research/architecture.bench.ts),
[host benchmarks](../benchmarks/compiler/host.bench.ts).

Browser WASM preparation of the standard CSS registry can exceed its 500ms hard
budget. Profile CSS loading and canonical index preparation, then reduce their
cost while preserving preparation once per revision. Recheck the preparation
median and uncached query percentiles with `vp run bench:compiler:browser`.

Sources: [browser measurement](../benchmarks/compiler/canonical.browser.test.ts),
[operation budgets](../benchmarks/performance/DEVELOPMENT.md#budget-policy).

## Registry context preparation

Creating the three [registry context](css-emission.md#registry-contexts) indexes took
about 2.5ms after a 32ms preparation of the package CSS in a native release build.
Measure the browser cost with `vp run bench:compiler:browser`.

Sources: [registry statements](../crates/design-system/src/registry/contexts.rs),
[theme index](../crates/design-system/src/theme/index.rs).

## Skipped CSS statements

Stylesheet lint reports [skipped statements](css-emission.md#skipped-statements) only
for the sources that a host marks for linting. Bundler integrations warn about every
skipped statement, but other hosts do not report a broken import or directive in package
CSS, such as a file under `node_modules`. Decide whether every host shows these records
as load warnings.

An import whose specifier the host cannot resolve has no file to watch, so creating the
file later does not reload the graph until another watched file changes.

The renderer still fails for a malformed `@stillwind-require-properties`, a requirement
without a `@stillwind-property` registration, and invalid `--theme()` syntax. Evaluate
skipping them with a lint finding as loading does.

Keyframe frames, view transition definitions, and `@apply` inside a utility body do not
check runtime conditions for each candidate. When their declarations cannot be lowered,
`invalid-runtime-condition` reports them at the whole module instead of the candidate.

Sources: [skipped statements](../crates/design-system/src/skipped.rs),
[stylesheet lint](../crates/compiler/src/compiler/stylesheet/lint/skipped.rs).

## Migration guide from Tailwind CSS

Users replace `@import "tailwindcss"` with `@import "stillwind"`, so the guide can describe
the other changes for that step. The guide needs these replacements:

- `@config` becomes the project's `tailwind` field.
- The `important` import modifier becomes the project's `important` field.
- `theme(...)` import modifiers stay unsupported. No plugin package in
  `e2e/plugins/tailwind` uses import modifiers. Define the replacement for a modifier on
  `@import "stillwind"`, whose theme the user cannot edit.
- `@source` and `source(...)` are removed, because Stillwind collects utilities from the
  modules that `files` selects.
- `blocklist` becomes the `no-restricted-utilities` lint rule.
- The `tw:` variant prefix is unsupported.

Decide whether the guide extends an existing document or needs its own file.
