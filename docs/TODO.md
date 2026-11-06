# Open work

This file contains unresolved design and validation work. Current supported
inputs and failure behavior are documented with their APIs. Each item below
identifies a remaining limitation and the evidence needed to resolve it.

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
entrypoints. Confirm invalidation when facts or asset support change without
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

## Stored variants with `style.when()`

Ref calls, `.in()`, and `.at()` must stand directly in `style()`, so every element that
needs the same ref condition repeats it. Add `style.when(variants)`, which stores a
variant sequence in a binding. Other `style()` calls can then use the binding as a key.

```tsx
const card = style.ref("card");
const cardHover = style.when(card("hover"));
const wideHover = style.when(`${card.at("width >= 40rem")}:${card("hover")}:md`);

style({ [cardHover]: "text-blue-600" });
style(`${cardHover}:underline`);
style({ [wideHover]: "grid-cols-2" });
```

Contract: a use of a stored variant produces the same styles and diagnostics as its
variant sequence written directly at the use. `` `${wideHover}:grid-cols-2` `` is equal
to `` `${card.at("width >= 40rem")}:${card("hover")}:md:grid-cols-2` ``.

Argument:

- The call takes one variant sequence, and all its variants must hold, as in one
  argument of `style.media()`. The sequence is one of these forms, with an optional
  final `:`:
  - a string literal or an interpolation-free template literal of variants, such as
    `"has-[[data-state=open]]:max-md"`;
  - a ref call or tag with optional `.in()` calls, or a `ref.at()` call or tag;
  - a template literal whose substitutions occupy complete variant segments, with the
    grammar of the variant part of a utility template;
  - another stored variant.
- An argument that is invalid in the direct form is invalid here with the same
  diagnostic. Examples are unknown variants, a receiver that is not a ref, `.at()`
  after `.in()`, and `if()` inside a ref wrapper.
- Several arguments are reserved for alternatives (OR), as in `style.media()`. Until
  the planned `or` variant defines how alternatives lower, a call with more than one
  argument is invalid.
- A call without arguments, a spread argument, a nonstatic argument, and extra
  arguments report a new lint, `invalid-when-argument`. A use of an invalid stored
  variant omits its input, as an invalid direct key does.

Declaration and value:

- Write the call as the initializer of a binding, as for `style.ref()`. A call inside a
  `style()` or CVA input reports `NestedMacroCall`.
- The value exists only at compile time, and the compiler replaces the call with
  `undefined`. The binding can be stored, exported, imported, passed, and returned like
  a ref declaration. Runtime code that reads it receives `undefined`.
- At a use, the value must resolve to exactly one declaration, because a key cannot
  choose its variant at runtime. A conditional choice such as `[c ? a : b]` is invalid.
  A ref receiver inside an argument keeps its finite candidates, as in a direct key.

Uses:

- A computed key in a `style()` object, and a complete variant segment followed by `:`
  in a utility template, in the positions that accept ref calls now.
- An argument of another `style.when()`.
- Not a `.in()` operand.
- CVA values reject computed keys, so they reject stored variants. Accepting a stored
  variant without a ref there is allowed but has low priority, because few use cases
  need it.

Diagnostics: report an argument error once, at the declaration, even when the binding
has no use or several uses. Report an error that depends on the use, such as
`unsatisfiable-variants` or `redundant-variant`, at the use, with the declaration as a
related location.

Implementation outline:

- Types: add `StoredVariant<Id>`, which extends `StyleVariantKey` so that TypeScript
  accepts it as a computed key and a template substitution. The checker supplies its
  identity through `StillwindDeclarationIdentity`, as for refs.
- Extraction: `computed_variant_entry` and the utility template scanner accept an
  expression whose candidates are one stored-variant declaration, and expand it into
  nested `StyleVariant` wrappers. The argument parser reuses the variant-segment
  grammar of utility templates.
- Modules: add an asset kind for stored variants. Its asset record carries the
  sequence as `StyleVariant` values that refer to refs by `IdentityId`, so an
  importing module can expand it. This changes the module summary schema.
- Editors: hover shows the expanded sequence, string arguments complete variants as
  keys do, and go to definition reaches the declaration.
- Tests: fixtures compare the CSS and diagnostics of the stored and direct forms.
  Generated class names may differ.

Sources: [ref types](../packages/core/src/macro/refs.ts),
[computed keys](../crates/js-oxc/src/expressions/class.rs),
[utility templates](../crates/js-oxc/src/templates.rs),
[asset records](../crates/semantic/src/assets.rs),
[macro contract](macro-contract.md#materialization-and-consumption-sites).

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
