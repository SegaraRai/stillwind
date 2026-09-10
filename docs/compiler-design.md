# stillwind compiler design

Status: original proposed architecture. For current crate ownership and host
boundaries, see [crate architecture](crate-architecture.md). This document turns the golden case into an
implementable compiler while preserving precise source locations for errors,
linting, quick-fixes, and a future formatter.

For the accepted framework projection and source-editing direction, see
[framework projection design](framework-projection-design.md). It supersedes the
future SFC frontend direction below: host-side projection, Oxc analysis, and
minimal Rust framework knowledge isolated in one file per framework.

## Decisions

1. The compiler core is a Rust workspace.
2. Oxc is the JavaScript / TypeScript / JSX frontend, not the universal
   frontend. Vue, Svelte, and Astro plug into a language-neutral extraction
   interface later.
3. Every semantic item carries an original `Span`; a location-free string is
   never sufficient compiler input.
4. `style()` calls are found by resolved import binding, never by scanning text
   for the word `style`.
5. The compiler emits edit plans and CSS plans. Thin adapters apply edits to
   source code and host build-tool lifecycles.
6. Formatter and linter consume the same parsed Stillwind IR. They do not recover
   intent from generated CSS.
7. Handle-producing APIs create declaration-site identities, while their values
   remain ordinary JavaScript values across module boundaries. TypeScript 7 is
   the primary propagation engine for aliases, conditionals, and module edges;
   Rust consumes typed facts and does not reimplement TypeScript data flow.

Oxc is a strong fit for the first frontend: it parses JS, TS, JSX and TSX,
exposes a span on every AST node, and provides semantic analysis for binding
resolution. Its AST is intentionally an efficient semantic representation, not
a lossless formatting tree; stillwind must retain original source text and its own
token spans. [Oxc parser](https://oxc.rs/docs/guide/usage/parser.html) and
[Oxc AST](https://oxc.rs/docs/contribute/parser/ast) describe these properties.

## Non-goals for v1

- Parsing arbitrary Vue, Svelte, or Astro files inside the Oxc frontend.
- A full JavaScript formatter.
- Evaluating arbitrary JavaScript at build time.
- Global text scanning for class-shaped strings.
- Emulating every Tailwind compatibility feature or arbitrary-value escape
  hatch.

Tailwind relationship primitives (`group`, `peer`, `container`, and container
variants) are nevertheless accepted as a migration layer. The optional
`forbidTailwindRelationships` lint policy reports `SHLINT001` so teams can move
to typed `style.ref()` / `style.scope()` relationships without reducing compiler
compatibility.

## System overview

```text
 source files + stillwind.css + dependency manifests
          │
          ▼
 Oxc extraction + declaration-site identity assignment
          │  ModuleIR: calls, static expressions, markup facts, exact spans
          ▼
 TypeScript 7 shadow project + checker sidecar
          │  span-keyed finite candidates and correlated ref/scope relations
          ▼
 semantic analysis
          │  theme lookup, selector grammar, ref/scope graph, lint diagnostics
          ├───────────────────────────┐
          ▼                           ▼
 CSS plan + CSS source map       source edit plan + runtime imports
          │                           │
          ▼                           ▼
 emitted CSS                  Vite/framework adapter applies edits
          │
          └──────────────► generated `.d.ts` and diagnostics
```

The core owns parsing of Stillwind semantics, validation, rule generation, naming,
incremental caching, and diagnostics. It does not own a bundler, filesystem
watcher, or framework renderer.

## Workspace and public boundaries

```text
crates/
  core/          # compilation orchestration and public Rust API
  source/        # SourceDb, spans, line indexes, diagnostics, edits
  js-oxc/        # JS/TS/JSX extraction using Oxc
  css/           # @theme extraction and CSS plan/source-map emission
  ir/            # language-neutral ModuleIR and StyleIR
  grammar/       # utility, selector-fragment, animation grammars
  semantic/      # theme resolution, ref/scope graph, lint rules
  codegen/       # deterministic class/symbol allocation, transform plan
  napi/          # napi-rs binding; no compiler logic
  cli/           # check, build, format and diagnostics rendering

packages/
  react/                # package CSS, types, and minimal runtime helpers
  vite/                 # thin Vite integration over the Node binding
  vue/                  # later: adapter contract only
  svelte/               # later: adapter contract only
```

`stillwind-core` accepts source buffers and compiler options and returns a pure
`CompileResult`. The Node binding and Vite plugin are replaceable transports;
they must not duplicate parsing or diagnostics logic.

The `stillwind` package exposes an aggregate CSS entry point plus `theme.css`,
`preflight.css`, `utilities.css`, and `builtins.css`. The aggregate entry point
imports all four. Individual entry points are independent: `builtins.css` must
be present for the compiler-owned utility, variant, and custom-property registry
to participate. The Rust crates embed these same CSS sources instead of
maintaining a second internal copy.
`tailwindcss` imports are not aliases for Stillwind and are passed to the host
resolver. The next CSS-entry migration will remove Rust embedding and the
remaining special treatment of Stillwind entry imports.

## Source model: locations are a first-class invariant

All offsets are UTF-8 byte offsets into immutable original source buffers.

```rust
pub struct FileId(u32);
pub struct BytePos(u32);

pub struct Span {
    pub file: FileId,
    pub start: BytePos, // inclusive
    pub end: BytePos,   // exclusive
}

pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

pub struct Origin {
    pub primary: Span,
    pub related: Vec<Span>,
}
```

`SourceDb` stores the original bytes, a line-start index, file identity, and a
lazy UTF-16 column index. Byte spans make slicing cheap and match parsers; the
UTF-16 index makes diagnostics correct in TypeScript editors and LSP clients.
The CLI displays line/column in Unicode-aware terminal coordinates.

Every input and emitted construct has provenance:

- `style()` call, each argument, and each condition expression;
- quote-delimited string literal, decoded-text segment, and each utility token;
- object key, attribute value, selector fragment, and computed handle key;
- handle declaration and every handle projection;
- theme token declaration and reference;
- generated class, declaration, rule, source-map segment, diagnostic, and fix.

### Escaped string literals

Source locations must not point at the wrong column after an escape sequence.
The JS frontend therefore produces `DecodedText`, not a bare `String`.

```rust
pub struct DecodedText {
    pub cooked: String,
    pub source: Span,               // including or excluding quotes is explicit
    pub chunks: Vec<TextChunk>,
}

pub struct TextChunk {
    pub cooked_range: Range<u32>,
    pub source_span: Span,
}
```

The utility lexer maps every token range through `chunks`. Diagnostics on
`bg-accnet`, for example, underline precisely that raw source slice even when
the containing literal contains escapes or non-ASCII text.

## Frontend contract

The rest of the compiler never sees an Oxc AST. A frontend returns this
language-neutral information:

```rust
pub trait Frontend {
    fn extract(&self, source: &SourceFile, options: &FrontendOptions)
        -> FrontendResult;
}

pub struct FrontendResult {
    pub module: ModuleIr,
    pub diagnostics: Vec<Diagnostic>,
}
```

`ModuleIr` contains macro bindings, `style` calls, handle/asset declarations,
and optional local markup relations. All nodes are `Spanned` and reference
original source text.

This is the boundary that keeps future framework support sane:

| File type                    | Initial parser              | What it contributes                             |
| ---------------------------- | --------------------------- | ----------------------------------------------- |
| `.js`, `.ts`, `.jsx`, `.tsx` | Oxc                         | macro calls, bindings, JSX ancestry facts       |
| `.vue`                       | Vue compiler/parser adapter | script calls plus template placement facts      |
| `.svelte`                    | Svelte parser adapter       | script calls plus template placement facts      |
| `.astro`                     | Astro parser adapter        | frontmatter calls plus template placement facts |

The framework parser owns host syntax and produces source spans in the original
single-file component. The stillwind core owns all styling semantics. No adapter
may turn calls into unspanned strings.

## Oxc frontend

### Macro binding detection

The Oxc frontend parses a module once, runs semantic analysis, and recognizes
only bindings imported from supported adapter entry points.

```ts
import { style } from "stillwind/react";
import { style as s } from "stillwind/react";
import * as stillwind from "stillwind/react";

style("p-4");
s("p-4");
stillwind.style("p-4");
```

Shadowed variables are ordinary JavaScript and are not transformed.

```ts
import { style } from "stillwind/react";
function example(style: (x: string) => unknown) {
  style("p-4"); // not a stillwind call
}
```

The current implementation recognizes stable `const` declarations in one
module:

```ts
const menu = style.scope("menu");
const trigger = style.ref(menu, "trigger");
const enter = style.animation({ from: "opacity-0", to: "opacity-100" });
const title = style.viewTransition({ old: "opacity-0", new: "opacity-100" });
```

That is not a typed-language rule. In TypeScript, these values may flow through
ordinary aliases, functions, properties, conditionals, imports, and re-exports;
the TS7 checker provides the resulting candidates. JavaScript without checker
facts deliberately remains limited to direct local declarations.

### Finite symbolic handle values

Handle producers are declaration-site assets, not local compiler variables.
The extractor assigns each producer a globally stable `IdentityId` from the
normalized module path, callsite span, kind, and compiler namespace. The same
identity is denoted if a producer call is evaluated repeatedly; this keeps the
CSS identity universe finite without restricting the resulting JavaScript value
from flowing normally.

```rust
enum AssetKind { Scope, Ref, Animation, ViewTransition }

struct IdentityId { kind: AssetKind, stable_key: StableKey }

enum SymbolicValue {
    Exact(IdentityId),
    Set(BTreeSet<IdentityId>),
    Universe(AssetKind), // a sound symbolic fallback, never a guessed value
}
```

`Set` is a value-domain representation at the Rust lowering boundary, not a
second propagation engine. TypeScript propagates branded declaration-site
identities through imports, exports, re-exports, aliases, conditionals, arrays,
objects, functions, and control flow. Rust receives the resulting facts keyed
by original source spans. JavaScript keeps direct, single-module support; typed
cross-module or conditional handles require TypeScript.

Refs carry a relationship table rather than a flattened `scope × ref` product:

```rust
struct RefAlternative {
    reference: IdentityId,
    permitted_scopes: SymbolicValue,
}
```

The planner may lower independent alternatives as `:is(...)`, but it must keep
the table when a scope/ref correlation matters. For example, it may emit a
selector list grouped by identical scope sets; it must not turn
`{ (scope_a, ref_a), (scope_b, ref_b) }` into four possible pairs.

Projections inherit their ref candidate set. `anchor`, `position`,
`anchorScope`, `container`, and `at()` therefore never need a separate identity
analysis. The same rule applies to animation and view-transition uses: they
refer to a producer set, while CSS names remain declaration-site stable.

### Module summaries and package manifests

Each module emits a small extraction summary containing producer identities,
metadata, exact call spans, and dependency specifiers. It does not contain a
Rust-computed cross-module value graph. The Vite adapter injects private phantom
identity arguments into an in-memory shadow source, asks TypeScript 7 for types,
and converts branded ref, scope, animation, and view-transition unions into
`TypeFact` records.

Source modules in the active build graph can be summarized directly. Published
packages expose the same versioned Stillwind manifest alongside their JS and
types. A missing manifest becomes a diagnostic only when Stillwind semantics
are required from that package; it never silently invents an identity.

When a value cannot be reduced to an exact local choice, generated code may use
an opaque runtime descriptor containing already-known identity metadata. The
descriptor selects pre-emitted marker classes or declaration classes at runtime
and is erased for exact values. It cannot parse, hash, allocate, or inject CSS.
`Universe(kind)` is the sound final fallback for the reachable build/package
closure; the CSS planner preserves it symbolically where possible and reports a
cost diagnostic rather than becoming unsound.

### Producer audit

| Producer / derivative | Identity and candidate rule                                                           | Runtime/CSS consequence                                                                   |
| --------------------- | ------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `scope()`             | declaration-site scope identity                                                       | descriptor selects an attached `.s_*` marker when dynamic                                 |
| `ref(scope?)`         | declaration-site ref plus correlated permitted scope set                              | selector planner preserves relation pairs; attachment selects `.s_*`                      |
| ref projections       | derived from the ref candidate set                                                    | exact uses erase; dynamic `position-anchor` / container uses select prebuilt declarations |
| `animation()`         | declaration-site keyframe asset                                                       | candidate use selects prebuilt `animation-name` declarations                              |
| `viewTransition()`    | declaration-site transition asset; `.for(key)` keeps its existing user key expression | every use shares one class; runtime identity/key populate the name and family properties  |

The audit is part of adding any future named CSS API: it must specify its
identity source, exported summary form, candidate algebra, correlation needs,
and exact-versus-descriptor code generation.

### Allowed static expression language

The extractor lowers only a deliberately finite subset.

```ts
style(
  "p-4", // string or no-substitution template literal
  active && "bg-accent", // logical guard
  active ? "block" : "hidden", // conditional guard
  ["rounded", compact && "p-2"],
  { class: ["legacy-button", loading && "is-loading"], "aria-busy": loading },
  { "&:hover": "bg-accent/90" },
  { [trigger`&:hover`]: "text-accent" },
);
```

The IR preserves guard expressions as source spans. The generated transform
reuses their original text; it does not serialize Oxc AST back into JavaScript.

Rejected examples include interpolated utility strings, computed non-ref
keys, object spreads outside the explicit `{ class: … }` value, unknown
function calls, and loop-derived collections. Object syntax is never a clsx
dictionary: external classes are opt-in through `class`, whose value is passed
unchanged to the clsx-compatible runtime helper.
The error is attached to the smallest unsupported expression and includes a
suggestion when a finite conditional form exists.

## Stillwind IR

The lowered IR deliberately distinguishes author intent from generated CSS.

```rust
pub struct StyleCall {
    pub adapter: AdapterKind,
    pub span: Span,
    pub inputs: Vec<StyleInput>,
}

pub enum StyleInput {
    Utilities { text: DecodedText, guard: Guard, origin: Origin },
    Attributes { entries: Vec<AttributeEntry>, guard: Guard, origin: Origin },
    SelfSelector { selector: SelectorFragment, body: Vec<StyleInput>, guard: Guard },
    RefSelector { reference: Ref, selector: SelectorFragment, body: Vec<StyleInput>, guard: Guard },
    ScopeAttachment { scope: Scope, origin: Origin },
    RefAttachment { reference: Ref, origin: Origin },
    Projection { reference: Ref, kind: ProjectionKind, origin: Origin },
    AnimationUse { animation: AnimationRef, options: DecodedText, guard: Guard },
    ViewTransitionUse { transition: TransitionRef, key: Option<ExprSpan>, guard: Guard },
}
```

`Guard` is a small boolean expression tree whose leaves are original JavaScript
expression spans. It supports correct runtime class selection without executing
user code.

```rust
pub enum ProjectionKind {
    Anchor,
    Position,
    Scope,
    Container { kind: ContainerKind },
    ContainerQuery { condition: ContainerCondition, body: UtilityList },
}
```

The ref/scope graph is semantic data, not a naming convention. A scope must be
attached to a strict ancestor of every bound ref. An unscoped ref targets its
own descendants; a scoped ref generates `scope:has(ref:where(selector))` and
may target any descendant of that scope.

## Utility and selector grammar

Do not copy Tailwind's full candidate grammar. Its candidate parser is a useful
reference for separating variants, modifiers, arbitrary forms, and candidates,
but stillwind should accept only its own typed vocabulary. Tailwind's implementation
also shows the value of retaining structured candidate/variant data rather than
handling utilities as undifferentiated strings. [Tailwind candidate parser](https://github.com/tailwindlabs/tailwindcss/blob/main/packages/tailwindcss/src/candidate.ts)

`stillwind-grammar` takes `DecodedText` and returns `Vec<Spanned<Utility>>`. Each
utility records variants, token family, token reference, modifiers, and exact
spans. A utility registry maps this representation to CSS declarations and the
theme namespace it may reference.

Selector keys are parsed by a separate restricted grammar:

- self selector: begins with `&`;
- ref selector: tagged template must begin with `&`;
- permitted pseudos, attribute selectors, `:has()`, and supported at-rules;
- no arbitrary descendant/global selector in the safe API.

The parser emits an AST, not concatenated selector strings. This is essential
for lints such as invalid `aria-*` value checks and for future selector-aware
formatting.

## Theme CSS frontend

`stillwind.css` is parsed into a `ThemeIndex` with spans on every token declaration
and selector block. The CSS parser is isolated behind `CssFrontend`; no
positionless CSS AST is allowed into the core.

The first implementation needs only correct parsing of imports, `@theme`,
declarations, and theme selectors such as `[data-theme="dark"]`. A parser spike
must verify declaration-level source locations before selecting the production
backend. The compiler can use a general CSS parser for grammar correctness, but
it owns the source-location adapter and its tests.

The theme index provides namespaces such as `color`, `spacing`, `breakpoint`,
`duration`, and `ease`; it also records the declaration span for diagnostics:

```text
Unknown token `accentt` in `bg-accentt`
  --> src/Button.tsx:14:17
  = did you mean `accent`?
  = `--color-accent` is declared at src/styles/stillwind.css:3:3
```

Tailwind v4 is useful reference material for CSS-first theme tokens and for its
separation of a scanner from utility/candidate parsing. stillwind differs by using
AST-backed macro extraction, because correctness and diagnostics matter more
than accepting class-like text anywhere in a project. [Tailwind repository](https://github.com/tailwindlabs/tailwindcss)

## Semantic analysis and linting

Semantic passes run after all source files for the compilation unit are lowered.
They return diagnostics and a validated `SemanticModule`, never mutate source.

Initial checks:

- unknown utility, token, variant, breakpoint, attribute state, or selector
  pseudo;
- invalid animation keyframe frame or animation option;
- invalid handle projection (`position` without a corresponding `anchor`,
  `at()` without `container()`);
- scope that cannot contain both relevant anchor-positioning participants;
- selector relationship impossible in locally visible JSX/template ancestry;
- a static view transition used in a repeated render;
- duplicate or contradictory declarations at the same selector/guard level;

Framework frontends may provide only partial structural facts. A lint rule must
report its confidence: it may error for locally proven misuse, warn for likely
misuse, and stay silent when a component/portal boundary makes a conclusion
unsound. The compiler never invents parent-child facts.

Diagnostics use stable codes (`SH001` etc.), primary and secondary labels,
help text, and optional non-overlapping edits.

```rust
pub struct Diagnostic {
    pub code: &'static str,
    pub severity: Severity,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
    pub fixes: Vec<Fix>,
}
```

## Code generation and transform plan

The CSS output is static. The only allowed runtime work is preserving existing
JavaScript control flow while choosing already-emitted class names and returning
adapter props.

The core returns textual edits rather than reprinting a module:

```rust
pub struct Edit {
    pub range: Span,
    pub replacement: String,
}

pub struct TransformPlan {
    pub edits: Vec<Edit>,
    pub required_runtime_symbols: RuntimeSymbols,
}
```

The Vite adapter applies non-overlapping edits with a source-map-aware editing
library, then injects any minimal adapter helper import. Retaining untouched
source means comments, user formatting, and unfamiliar syntax survive exactly.

For a fully static call, the replacement can be a plain object literal. For
conditional classes, React's adapter may use a tiny `stillwindClsx` helper:

```tsx
{
  className: stillwindClsx("_s_a1", loading && "_s_b2"),
  "aria-busy": loading || undefined,
}
```

This is not a CSS-in-JS runtime: it does not parse, allocate, inject, or hash
CSS. CSS rules and all generated names already exist in the build output.

### Rule units and cascade correctness

The compilation unit is a callsite branch, not a global atomic declaration.
Within a branch, declaration order is preserved. Conditional branches generate
their own class/rule units and are emitted after their base unit for that
callsite.

This avoids the incorrect assumption that HTML class attribute order controls
CSS cascade order. It also makes a future duplicate-declaration lint possible.
Exact rule deduplication is an optimization to add only when it preserves this
ordering contract; it is not a v1 requirement.

Names are deterministic and private:

- class names derive from canonical rule content and the callsite identity;
- handle names derive from normalized module identity plus lexical binding
  identity;
- keyframe names derive from canonical keyframe content and lexical identity;
- static view-transition names derive from their declaration identity.

The naming module is the sole owner of hashes, identifier escaping, collision
handling, and CSS namespace prefixes.

Stillwind-owned identifiers use the configurable `--sw-` prefix. Internal
custom properties retained for Tailwind compatibility use the separately
configurable `--tw-` prefix; the two namespaces must remain distinct.

Typed registrations in the CSS registry use `@stillwind-property`. The
stylesheet emitter resolves these definitions after theme CSS, authored CSS,
and generated utilities have been combined, then lowers only referenced names
to standard `@property` rules. Authored standard `@property` rules are retained
verbatim and suppress a compiler-owned registration with the same final name.

Every generated CSS declaration carries an `Origin`; the CSS emitter writes a
source map back to the style call and, where useful, to the exact utility token.

## Formatter architecture

`stillwind format` is initially a macro formatter, not a JS formatter. It uses
the frontend to find `style()` ranges and uses `StyleIR` plus original source
gaps to produce edits only inside those ranges.

v1 formatter rules:

- normalize whitespace inside utility strings;
- one utility per canonical token spelling;
- preserve argument ordering and all semantic utility order;
- preserve guard expressions and attribute value expressions verbatim;
- format selector and animation frame maps with ordinary, localized edits;
- never reorder potentially overlapping declarations automatically.

Because every token and delimiter is spanned, a formatter or quick-fix can
replace only the intended range. A full source formatter, should one ever be
desired, is a separate lossless-syntax project and must not be built on a
location-only AST.

## Incrementality

Cache keys include:

- file contents and frontend kind;
- adapter mode and compiler version;
- reachable theme CSS contents and imports;
- relevant package/versioned utility registry.

Cache per-file `ModuleIr`, lowered `StyleIR`, diagnostics, and CSS fragments.
A source edit recompiles only that module. A theme edit invalidates semantic
resolution and CSS fragments that depend on it, and regenerates `.d.ts`.

Shadow inputs and extracted producer records are additionally content-addressed.
Changing an exported identity invalidates TypeScript dependants; changing an
unrelated style call does not. Checker objects are snapshot-local and never
stored in the Rust cache.

The Vite plugin owns watch registration and HMR invalidation; the Rust core is
pure and testable without Vite.

## TypeScript-backed propagation plan

1. Keep producer recognition, original spans, CSS semantics, and code generation
   in Rust/Oxc.
2. Maintain one long-lived TypeScript 7 checker sidecar per Vite project and
   update its virtual shadow files incrementally. The Vite plugin now owns this
   lifecycle; it closes the service with the dev server or build bundle.
3. Batch candidate queries and immediately convert checker objects into the
   language-neutral `TypeFact` representation.
4. Treat missing, widened, forged, or failed checker results conservatively;
   never restore a parallel Rust abstract interpreter as a fallback.
5. Extend the same manifest-backed identity channel to packages whose sources
   are unavailable.

Conditional animations lower to candidate CSS classes plus a runtime identity
lookup. Static and conditional view transitions, whether keyed or unkeyed, share
one generated class. Their identity and optional key are evaluated once by the
runtime style helper, which populates the configured view-transition name and
family custom properties.

Every phase retains the invariants that CSS is fully generated before runtime,
source locations remain exact, generated identities are collision-checked, and
the final selector/declaration plan is semantically equivalent to the symbolic
candidate relation.

The synchronous TS7 API serializes requests, so a snapshot cannot race another
generation inside one plugin instance. Each update replaces the virtual file
set, reports changed/created/deleted paths, creates the next snapshot, and only
then disposes the previous snapshot. If the checker fails, the service closes
the process and retries once from a cold snapshot; a second failure is surfaced
as a compiler integration error rather than falling back to Rust propagation.

## Test strategy

1. Grammar unit tests: utility and selector ASTs, including exact spans.
2. Frontend fixtures: imports, shadowing, JS/TS/JSX syntax, unsupported forms.
3. Snapshot fixtures: input source, diagnostics with labels, transform output,
   CSS output, and source maps.
4. Property tests: lexer/parser never panic; formatter is idempotent; edits do
   not overlap; name allocation is deterministic.
5. Framework contract tests: each frontend fixture produces equivalent
   language-neutral IR for equivalent author intent.
6. Browser integration tests later: generated CSS behavior for anchors,
   container queries, animations, and view transitions.
7. TypeScript-fact fixtures: exported values, re-exports, conditionals,
   correlation preservation, and exact/dynamic descriptor erasure.
8. Property tests: shadow/original span mapping is stable, relation factoring
   never adds a ref/scope pair, and invalid facts fail conservatively.

## Implementation order

1. `stillwind-source`, diagnostics renderer, test-fixture harness, and span/source
   map invariants.
2. Oxc import/binding resolution and `style("...")` extraction with exact token
   spans.
3. CSS `@theme` index, typed utility registry, diagnostics, and CSS emission.
4. React transform plan plus Vite adapter; support static strings and finite
   conditional branches.
5. Attributes and self selectors.
6. `style.ref()` / `style.scope()` graph, selector relations, anchor positioning, and
   container queries with lint rules.
7. String-based `style.animation()` and named view-transition assets.
8. `.d.ts`, CLI `check`, macro formatter, and editor/LSP integration.
9. Vue, Svelte, and Astro frontends once their extractor contracts are proven.

At each step, fixture diagnostics and source-location tests are a release gate.
No feature is considered implemented if it emits correct CSS but points users at
the wrong character when it fails.
