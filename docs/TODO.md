# Open work

This file contains unresolved design and validation work. Current supported
inputs and failure behavior are documented with their APIs. Each item below
identifies a remaining limitation and the evidence needed to resolve it.

## Tailwind compatibility gaps

The compatibility baseline is Tailwind CSS 4.3.5. These contracts still need work:

- Built-in color utilities resolve numeric alpha modifiers without looking up
  `--opacity-*` tokens. Define named opacity support and verify its precedence
  against numeric modifiers, including inline theme values and canonicalization.
- Arbitrary breakpoint variants such as `max-[64rem]:flex` and selector variants
  such as `[&_::before]:flex` remain unchanged instead of using the named or child
  variants that Tailwind suggests. Verify equivalent output before adding replacements.
- Canonical collapse picks `text-z` for `text-foreground text-primary` when all three
  custom utilities emit `color: red`. Decide whether input candidates take precedence
  over shorter aliases. Measure index preparation with thousands of
  functional variant values and collapse with many utilities linked through `sr-only`;
  the independent implementations do not establish equivalent performance.
- Nesting lowering rejects implicit `@scope` roots inside style rules. Preserve their
  stylesheet-dependent root before enabling this transformation.

Sources: [Tailwind 4.3.4](https://github.com/tailwindlabs/tailwindcss/releases/tag/v4.3.4),
[Tailwind 4.3.5](https://github.com/tailwindlabs/tailwindcss/releases/tag/v4.3.5),
[plugin theme](../packages/host/src/css/plugins/tailwind/theme.ts),
[plugin value inference](../packages/host/src/css/plugins/tailwind/data-type.ts),
[canonical candidates](../crates/compiler/src/query/canonical/candidate/functions.rs),
[variant bodies](../crates/design-system/src/evaluation/variants/body.rs),
[utility rendering](../crates/compiler/src/compiler/utilities/render.rs).

## Type facts under Deno on Linux

The synchronous TypeScript 7 API passes `child.stdin._handle.fd` to `fs.writeSync` on
POSIX systems. Under Deno, this value is not an operating system file descriptor, so
the Fresh fixture fails with `Bad file descriptor (os error 9)`. macOS uses the same
code path, so the fixture expects this failure there, but this needs testing. Find a Deno or TypeScript API that exchanges checker
messages synchronously, then remove `platformFailure` from the Fresh fixture and return
it to the `primary` tier.

Sources: [checker](../packages/host/src/semantics/node/checker.ts),
[Fresh fixture](../e2e/frameworks/fresh2-vite7-preact10/fixture.ts).

## Conditional utility completions

Utility strings in `style()` conjunctions, such as `style(enabled && "opacity-50")`,
still suggest their own inferred strings. Evaluate suppression while preserving ref
template inputs and finite macro choices.

Sources: [style declaration](../packages/core/src/macro/style.ts),
[input checks](../packages/core/src/macro/input.ts).

## Semantic fingerprints

Semantic revisions hash `Debug` representations of facts and asset metadata in
several entrypoints. Define one deterministic representation shared by those
entrypoints. Confirm invalidation when facts or asset metadata change without
making incidental debug formatting part of the cache contract.

Sources: [project input](../crates/napi/src/api/project.rs),
[session facts](../crates/napi/src/session/facts.rs),
[compiler API](../crates/compiler/src/compiler/api.rs).

## Selective CSS reloads

Definition dependencies remain conservative, and successful CSS reloads recompile
retained analyses. Selective invalidation needs dependencies for successful
resolution, failed lookup, new definitions, and namespace resets. Verify added,
changed, and removed definitions before using narrower reload selections.

Sources: [module compiler](../crates/compiler/src/module_compiler.rs),
[project session](../crates/project/src/session/state.rs).

## Analysis work after source preparation

- Load dependencies found by the checker in batches before collecting type facts.
  Each entry must still resolve its own required imports and report missing inputs in its project.
- Reuse completed native analyses when entries share dependencies. Generate code, CSS,
  and full editor indexes only when a caller requests them. Lint and formatting must
  still validate assignments and declaration identities.
- Check cancellation between dependency batches and merge pending requests for the
  same revision. Later edits must not change editor answers that have already been returned.
- Keep `SourceFormatter` project state when its inputs have not changed. Reload it when
  configuration dependencies change. Changes outside open buffers currently reopen the workspace.

Use the existing project queues and prepared sources. These improvements should not
require a new graph system, persistent caches, extra workers, or a daemon.

## Assets from compiled packages

Assets whose declaration source is absent from the graph have no supported
versioned package manifest channel. Define identity records, value-shape versions,
naming revisions, CSS ownership, and collision reservations before accepting them.
Keep checker propagation separate from the delivery of actual asset definitions.

Sources: [module graph](../packages/host/src/compilation/module-graph.ts),
[semantic values](../crates/semantic/src/symbolic.rs).

## Libraries built with Stillwind

The contract for a library that uses Stillwind is undecided. The expected direction is
that a library ships compiled `style()` calls, so its consumers do not transform the
package. Build integrations already skip files under `node_modules`. Decide how a
library builds and publishes its output, how its CSS reaches the consumer, and how its
theme tokens and generated names coexist with the consumer's. Keep this usage out of
the public documentation until the contract is decided.

Sources: [Vite transform](../packages/integration/src/vite/transform.ts),
[webpack rules](../packages/integration/src/webpack/rules.ts).

## View-transition keys

The linter counts transition-name suppliers but does not detect repeated-render
or unstable-key use. Establish which framework facts can support actionable
warnings without treating valid cross-component pairing as a collision.

Source: [view-transition analysis](../crates/compiler/src/style_analysis/view_transition.rs).

## Nearest-ancestor ref conditions

A ref condition matches when any ancestor with that ref matches. When a recursive
component renders the same ref inside itself, a state on an outer element therefore
reaches the descendants of inner elements. Add `.until(limit)` to bound the region
of a ref, and lower it to a scope with a limit (`@scope (root) to (limit)`). No
fallback without `@scope` is planned.

Syntax and meaning:

- `.until()` belongs to the ref that starts the bounded region.
  `row("hover").until(row)` matches when the nearest `row` ancestor of the target is
  hovered. `trigger("hover").in(menu.until(menu))` requires the trigger and the target
  to be in the same nearest `menu`.
- `limit` accepts only a raw ref, because the limit tests structure, not state.
- The limit element itself is outside the region. A target that carries the limit ref
  never matches, and a lint should report this when the same call attaches that ref.
- `.until()` after `.in()`, such as `.in(menu).until(menu)`, is not supported. When a key
  uses `.in()`, its subject ref is not an ancestor of the target, so
  `trigger("hover").until(x).in(menu)` is rejected as a key.
- `.at()` does not accept `.until()`, because a container name already resolves to the
  nearest container.
- Repeated `.in()` calls keep their current meaning: an AND of conditions whose order
  does not matter. A scope operand may itself use `.in()` to express nesting:
  `a("s").in(b.in(c))` requires `c` to contain `b`, and `b` to contain `a` and the
  target. Bounded scopes can nest only through this operand form, because the compiler
  cannot determine the order of `@scope` blocks for unrelated flat `.in()` operands.

Lowering, as verified in Chromium 152:

- Without `.in()`, put the state in the root selector and repeat the unbounded
  selector inside an inner scope of the target:
  `@scope (:where(.row):hover) to (:where(.row)) { @scope (.T) { :scope:is(:where(.row):hover *) { … } } }`.
  The repeated condition is always true at that point, and it gives the rule the same
  specificity as the unbounded form. The scope selectors add no specificity.
- With `.in()`, put the trigger-side bound in the prelude of the inner scope, because
  `:scope` refers to the parent scope root only there:
  `@scope (:where(.menu)) to (:where(.menu)) { @scope (:where(:scope):has(:is(:where(.trigger):hover):not(:scope :where(.menu) *)) .T) { :scope:is(:where(.menu):has(:is(:where(.trigger):hover)) *) { … } } }`.
  In the prelude of the outer scope, `:scope` refers to the document root, and the
  rule never matches.

Cascade:

- The inner `@scope (.T)` gives every bounded rule a scope proximity of 0. Without it,
  the rule whose root is nearer would win over a later rule with the same specificity.
- A rule without a scope has infinite proximity. A bounded rule therefore wins over an
  unscoped rule with the same specificity regardless of order, for example over a
  derived style composed after a base with a bounded key. Where Stillwind guarantees
  order (one call, composition, and unit order inside a module), wrap a later rule
  that conflicts on a property in `@scope (.T) { :scope… }`. This keeps its
  specificity and makes order decide again. Document the tie rule for other CSS.

Types: `refs.ts` separates each stage by `refVariantKind`. Add a `bounded` stage, and
let `RefScope` accept `ScopedRefVariant` and bounded operands. Give `ScopedRefVariant`
a parameter that records whether its subject is bounded, so `RefScope` can accept that
form while keys reject it. TypeScript cannot constrain computed keys, so the compiler
must also diagnose a bounded subject in a key. Decide whether `ScopeIds` needs a tree
for nested operands or only a set of identities. Collect the stages, their methods, and
the positions that accept them in one table in the macro contract.

Remaining validation: test the lowering in Firefox and Safari, and test the
interaction of conditional rules that use `revert-rule` with scoped rules.

Sources: [ref types](../packages/core/src/macro/refs.ts),
[macro contract](macro-contract.md#ref-attachments-and-providers),
[ref declarations](../crates/js-oxc/src/visit.rs).

## Stored variant follow-ups

`style.when()` stores a variant sequence or alternatives. These parts are not
designed or built yet:

- Editors: hover over a use should show the expanded sequence, string arguments
  should complete variants as keys do, and go to definition should reach the
  declaration.
- CVA: CVA values reject computed keys, so they reject stored variants. Accepting a
  stored variant without a ref there is possible but has low priority, because few
  use cases need it.
- Related locations: a finding for an imported declaration points at the use. It
  could also name the declaration as a related location.

Sources: [extraction](../crates/js-oxc/src/stored_variants.rs),
[macro contract](macro-contract.md#stored-variants).

## Variant alternative follow-ups

`or(...)` expands to one candidate for each sequence. These parts are open:

- Nested values: `not-or(a,b)` could resolve as `not-a:not-b`, and `group-or(a,b)`
  as the alternatives of `group-a` and `group-b`. They report `invalid-variant` now.
- Media lists: `style.media("or(md,print)")` could produce the media query list
  `(width >= 48rem), print`. It reports `invalid-variant` now.
- Duplicates: `or(hover,hover)` emits the same rule twice. Only
  `prefer-canonical-utilities` reports it.
- Editors: completion and hover inside `or(...)` should treat each sequence as a
  variant prefix. A finding for an expanded candidate covers the whole candidate,
  not the failing sequence.

Sources: [expansion](../crates/compiler/src/compiler/style_variants/alternatives.rs),
[macro contract](macro-contract.md#variant-alternatives).

## Remaining documentation capabilities

The utility explorer searches the default registry's finite completion catalog.
Add on-demand resolution for numeric and arbitrary candidates without enumerating
infinite domains. Show rule previews and required document support separately.
Exact CSS equivalence needs a public query contract that preserves selectors,
conditions, declaration order, and support requirements; property/value term
searches cannot prove that equivalence.

Macro references publish actual compiled code and generated CSS. A display of
each optimization pass requires compiler trace data; the current compiler has no
public pass-selection or trace API. Preserve the distinction between compiler
lowering and later bundler minification.

Compiler codes can change before the first release. Freeze published identities
or record redirects and retirement pages before tooling exposes permanent
links. Removed codes must not silently describe a different error.

Sources: [utility query API](../packages/native/binding.d.ts),
[canonical normalization](../crates/compiler/src/query/canonical/index.rs),
[runtime forms](runtime.md#lowering-and-materialization),
[compiler error codes](../crates/diagnostics/src/compilation/code.rs),
[authored reference pages](../apps/website/src/content/docs/docs/macros/style.mdx).

## Astro cache in CI

Astro's environment loading registers CI metadata as Vite Task dependencies.
Changing `ACTIONS_ORCHESTRATION_ID` or `GITHUB_RUN_ID` invalidates an otherwise
unchanged Astro task, including when `cache.untrackedEnv` lists the variable.
Exclude runner metadata from cooperative tracking while preserving environment
variables that affect generated files. Verify reuse across separate CI runs.

Sources: [site task caches](../tools/workspace/task-cache.ts),
[cooperative tracking](https://viteplus.dev/guide/automatic-data-tracking).

## Measurement coverage

`TypeScriptFactsService` enables checker timing collection but exposes only
process, snapshot, and filesystem counters. Expose round trips, transferred bytes,
and server time in host debug telemetry so checker work can be distinguished
from transport and host work.

Source: [checker service](../packages/host/src/semantics/node/service.ts).

Synthetic build/watch benchmarks and reused checker workloads do not establish
latency for a typed application through filesystem events, subsequent module
requests, browser CSS application, and framework HMR. Define and measure that
complete path before assigning application latency guarantees.

Sources: [architecture benchmarks](../benchmarks/research/architecture.bench.ts),
[host benchmarks](../benchmarks/compiler/host.bench.ts).

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

## Lit server rendering

The element directive applies props in the browser. Add an SSR path that emits the
same classes and inline values before adding Lit server acceptance.

Source: [Lit directive](../packages/core/src/frameworks/lit.ts).

## Source maps of combined development CSS values

With `devSourceMaps`, each fragment of an explicit CSS value embeds its own inline map,
and the value module joins the fragments with `cssText(parts)`. A browser reads only the
last `sourceMappingURL` comment, so locations in a value with more than one part map to
the wrong source. Compose the fragment maps into one map for the joined text, as
`renderCssWithSourceMap` does for the development runtime. `styleSheets` creates one sheet
for each part and is not affected.

Sources: [virtual modules](../packages/integration/src/vite/virtual-modules.ts),
[CSS text](../packages/core/src/css.ts).

## Documentation for coding agents

Agents install the skills and add `AGENTS_TEMPLATE.md` to the project by hand. Decide
whether a CLI command should link the skills into the agent's directory and update the
project's instruction file, as `void init` does.

The migration skill assumes that a project migrates in batches. Confirm whether Tailwind
CSS and Stillwind can build the same application at the same time, and describe that
setup in the migration guide if they can. Measure how well agents follow the skills
with evaluations on sample projects.

Sources: [skills](../packages/core/skills/),
[agent page](../apps/website/src/content/docs/docs/agents.md),
[package documentation generator](../apps/website/package-docs.ts).

## Complete design-system searches in the CLI

`stillwind utilities 'p-*'` reads the completion catalog, which omits numeric and arbitrary
values, so `p-4` is not in the result. Expose the shape of functional utilities from the
compiler, such as `p-<number>`, and show it in search results. `text-2xl--line-height` also
appears in searches, because the `text-*` utility accepts that token. Decide whether a search
should hide such tokens.

`stillwind theme` shows values as written. Decide whether it should also show resolved values,
as preview comments do.

Sources: [design-system queries](../packages/host/src/editor/design-system.ts),
[candidate completion](../crates/compiler/src/query/completion/candidates.rs).

## Retried React Router resolutions

Rolldown can pass the `custom` option of one `this.resolve()` call to a different call that runs
at the same time ([rolldown#11180](https://github.com/rolldown/rolldown/issues/11180)). Stillwind
resolves server-only sources with the flag that skips React Router's `.server` check, so it
resolves again when React Router reports `Server-only module referenced by client`. After a
Rolldown release gives each pending call its own key, update Vite+ and remove the retry.

Sources: [server source resolution](../packages/integration/src/vite/server-source-resolve.ts),
[React Router fixture](../e2e/frameworks/vite8-react19-reactrouter8/fixture.ts).

## Separate module compilation from linking

The output of a module should depend only on its own source, the checker facts for
its own probes, and the interfaces of the modules that those facts name. An edit
that keeps the interface of a dependency should not compile, transform, or query
facts for its consumers again. Linking should resolve the names that cross module
boundaries.

Each declaration has a key that survives edits elsewhere in its module: kind,
module, enclosing declaration path, ordinal, and binding name. Checker identities
and cross-module references use this key, and the names of ref markers, anchors,
containers, keyframes, and view-transition tokens derive from it. Each `style()`
call has a key of the same form, and its utility classes derive from that key. An
edit that only moves declarations or calls therefore changes no facts of a consumer
and no generated names. These couplings remain:

- The JavaScript of a consumer contains names from the dependency:
  - A literal ref attachment writes the ref marker as the class name.
  - The identity of a consumer's own utility class spells imported refs, stored
    variants, and containers by their keys and labels. It still includes the
    generated names of imported anchors, keyframes, and view transitions, which
    change only when the key, the label, or the naming configuration changes.
  - The candidate tables of conditional anchors and keyframes use the imported
    names as keys.
  - A dynamic view-transition name passes the imported token as a literal. It does
    not read the token from the receiver.
- The CSS of a consumer contains the imported names, and the artifact key contains
  the imported identities. Planning adds the rules of imported keyframes and view
  transitions from the artifact of the declaring module.
- Compilation reads the inline-style flags of composed bases from the analyses of
  the whole batch, not from the interface of the dependency.
- Composition records only the base key in `compositionTopology`. The consumer's
  JavaScript and CSS stay the same. Conditional ref attachments read `.c` at
  runtime and also stay the same. An edit of a utility string after the last
  declaration changes no consumer output.

Each compilation still sends the semantic closure of its entry to the native
session. The session reuses the stored analysis of a module whose source buffer,
input, and facts are unchanged, without scanning or finalizing it again. Most of the
remaining native compilation time is the first compilation of each module.

A development server does not register dependencies with `addWatchFile`. Its hot
update hook compiles the changed producer and then compares the interface of the
changed file before and after the edit: the asset records of its scan and the types
of its exports as facts decode them. Utility strings in a style output type do not
take part. When the interface is the same, the hook does not compile the producers
that read the file. Otherwise it compiles them again and sends a hot update only for
those whose JavaScript changes. The comparison does not cover these cases, so a
development server can keep stale results until the consumer compiles again:

- Checker diagnostics and Stillwind lint findings of a consumer.
- A producer type that the edit adds or removes and that changes how facts classify
  a consumer type that the exports do not reach.
- Global declarations and module augmentation in the changed file.

A changed file that is not itself a producer always compiles its consumers again.
Vite environments share one checker, but each environment has its own native session
and compiles every module again. In a build the checker installs the discovered
sources once and keeps one snapshot. Its time goes to opening that snapshot,
registering producer types, reading the semantic diagnostics of each queried file,
and reading facts.

Planned direction:

1. Compile one module for each transform with the facts of its own probes. Key
   artifacts by the dependency interfaces that they read, and move cycle and
   missing-producer checks to linking. Read the inline-style flags of bases from
   the interface of the dependency. Style-effect lints such as
   `interfering-imported-style-base` already run over published artifacts and stay
   there. Unchanged dependencies already reuse their stored analyses, so measure
   the remaining gain against the cost of reporting composition errors at link
   time.

Open decisions: whether the checker diagnostic check stays on the compilation
path, and how development reports link-time errors.

Sources: [checker sources](../packages/host/src/semantics/checker-source.ts),
[variant identities](../crates/compiler/src/compiler/style_variants/plan.rs),
[composition checks](../crates/ir/src/analysis.rs),
[stylesheet plans](../crates/project/src/stylesheet_plan.rs),
[artifact keys](../crates/compiler/src/module_compiler.rs),
[module graph](../packages/host/src/compilation/module-graph.ts),
[Vite compilation](../packages/integration/src/vite/compile.ts),
[Vite hot updates](../packages/integration/src/vite/development.ts),
[export interfaces](../packages/host/src/semantics/node/export-interface.ts),
[native compile batches](../crates/napi/src/session/project.rs).

## Document CSS in Turbopack worker processes

Turbopack runs loaders in a pool of worker processes. Each process keeps its own
compiler for document CSS and compiles only the modules whose sources, or the
sources of the local modules that they import, changed since its previous
reconstruction. Turbopack chooses the process for each reconstruction, so the first
edit that reaches a process without a previous reconstruction compiles the whole
entry graph. The compilers of a process load the project files into one checker
and watch the application directory, so such a compilation queries a warm checker.
In a generated Next application with about 270 modules, an edit takes about 2
seconds.
Decide whether reconstruction should reuse compiled modules across processes, for
example through a cache on disk, or whether the document CSS should combine
per-module results instead of compiling the graph in one process.

Sources: [graph reconstruction](../packages/integration/src/turbopack/project.ts),
[application benchmark](../benchmarks/apps/run.ts).
