# stillwind compiler design

Status: proposed architecture. This document turns the golden case into an
implementable compiler while preserving precise source locations for errors,
linting, quick-fixes, and a future formatter.

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

Oxc is a strong fit for the first frontend: it parses JS, TS, JSX and TSX,
exposes a span on every AST node, and provides semantic analysis for binding
resolution. Its AST is intentionally an efficient semantic representation, not
a lossless formatting tree; shorth must retain original source text and its own
token spans. [Oxc parser](https://oxc.rs/docs/guide/usage/parser.html) and
[Oxc AST](https://oxc.rs/docs/contribute/parser/ast) describe these properties.

## Non-goals for v1

- Parsing arbitrary Vue, Svelte, or Astro files inside the Oxc frontend.
- A full JavaScript formatter.
- Evaluating arbitrary JavaScript at build time.
- Global text scanning for class-shaped strings.
- Emulating every Tailwind compatibility feature or arbitrary-value escape
  hatch.

## System overview

```text
 source files + stillwind.css
          │
          ▼
 language frontend (Oxc first; framework extractors later)
          │  ModuleIR: calls, static expressions, markup facts, exact spans
          ▼
 Stillwind expression lowering
          │  StyleIR: utility groups, guards, attrs, selectors, projections
          ▼
 semantic analysis
          │  theme lookup, selector grammar, handle graph, lint diagnostics
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
  shorth-core/          # compilation orchestration and public Rust API
  shorth-source/        # SourceDb, spans, line indexes, diagnostics, edits
  shorth-js-oxc/        # JS/TS/JSX extraction using Oxc
  shorth-css/           # @theme extraction and CSS plan/source-map emission
  shorth-ir/            # language-neutral ModuleIR and StyleIR
  shorth-grammar/       # utility, selector-fragment, animation grammars
  shorth-semantic/      # theme resolution, handle graph, lint rules
  shorth-codegen/       # deterministic class/symbol allocation, transform plan
  shorth-node/          # napi-rs binding; no compiler logic
  shorth-cli/           # check, build, format and diagnostics rendering

packages/
  react/                # types plus minimal runtime helpers
  vite/                 # thin Vite integration over the Node binding
  vue/                  # later: adapter contract only
  svelte/               # later: adapter contract only
```

`shorth-core` accepts source buffers and compiler options and returns a pure
`CompileResult`. The Node binding and Vite plugin are replaceable transports;
they must not duplicate parsing or diagnostics logic.

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
single-file component. The shorth core owns all styling semantics. No adapter
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
  style("p-4"); // not a shorth call
}
```

For v1, macro declarations must be stable `const` bindings in the current
module. The compiler recognizes:

```ts
const trigger = style.handle();
const enter = style.animation({ from: "opacity-0", to: "opacity-100" });
const title = style.viewTransition({ old: "opacity-0", new: "opacity-100" });
```

Reassignments, captures through arbitrary functions, dynamic property access,
and cross-module re-exports are diagnosed as unsupported rather than guessed.
Support for safe exported contracts can be added after the module graph API is
designed.

### Allowed static expression language

The extractor lowers only a deliberately finite subset.

```ts
style(
  "p-4", // string or no-substitution template literal
  active && "bg-accent", // logical guard
  active ? "block" : "hidden", // conditional guard
  ["rounded", compact && "p-2"],
  { "aria-busy": loading },
  { "&:hover": "bg-accent/90" },
  { [trigger`&:hover`]: "text-accent" },
);
```

The IR preserves guard expressions as source spans. The generated transform
reuses their original text; it does not serialize Oxc AST back into JavaScript.

Rejected examples include interpolated utility strings, computed non-handle
keys, object spreads, unknown function calls, and loop-derived collections.
The error is attached to the smallest unsupported expression and includes a
suggestion when a finite conditional form exists.

## Shorth IR

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
    HandleSelector { handle: HandleRef, selector: SelectorFragment, body: Vec<StyleInput>, guard: Guard },
    Projection { handle: HandleRef, kind: ProjectionKind, origin: Origin },
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

The handle graph is semantic data, not a naming convention. It links each
`anchor`, `position`, `scope`, `container`, `at`, and selector use to the one
definition that created the handle.

## Utility and selector grammar

Do not copy Tailwind's full candidate grammar. Its candidate parser is a useful
reference for separating variants, modifiers, arbitrary forms, and candidates,
but shorth should accept only its own typed vocabulary. Tailwind's implementation
also shows the value of retaining structured candidate/variant data rather than
handling utilities as undifferentiated strings. [Tailwind candidate parser](https://github.com/tailwindlabs/tailwindcss/blob/main/packages/tailwindcss/src/candidate.ts)

`shorth-grammar` takes `DecodedText` and returns `Vec<Spanned<Utility>>`. Each
utility records variants, token family, token reference, modifiers, and exact
spans. A utility registry maps this representation to CSS declarations and the
theme namespace it may reference.

Selector keys are parsed by a separate restricted grammar:

- self selector: begins with `&`;
- handle selector: tagged template must begin with `&`;
- permitted pseudos, attribute selectors, `:has()`, and supported at-rules;
- no arbitrary descendant/global selector in the safe API.

The parser emits an AST, not concatenated selector strings. This is essential
for lints such as invalid `aria-*` value checks and for future selector-aware
formatting.

## Theme CSS frontend

`shorth.css` is parsed into a `ThemeIndex` with spans on every token declaration
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
  = `--color-accent` is declared at src/styles/shorth.css:3:3
```

Tailwind v4 is useful reference material for CSS-first theme tokens and for its
separation of a scanner from utility/candidate parsing. shorth differs by using
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
- forbidden `unsafe` use or missing `reason`.

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

The Vite plugin owns watch registration and HMR invalidation; the Rust core is
pure and testable without Vite.

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

## Implementation order

1. `shorth-source`, diagnostics renderer, test-fixture harness, and span/source
   map invariants.
2. Oxc import/binding resolution and `style("...")` extraction with exact token
   spans.
3. CSS `@theme` index, typed utility registry, diagnostics, and CSS emission.
4. React transform plan plus Vite adapter; support static strings and finite
   conditional branches.
5. Attributes and self selectors.
6. `style.handle()` graph, selector relations, anchor positioning, and
   container queries with lint rules.
7. String-based `style.animation()` and named view-transition assets.
8. `.d.ts`, CLI `check`, macro formatter, and editor/LSP integration.
9. Vue, Svelte, and Astro frontends once their extractor contracts are proven.

At each step, fixture diagnostics and source-location tests are a release gate.
No feature is considered implemented if it emits correct CSS but points users at
the wrong character when it fails.
