# TypeScript type facts for SFCs

Stillwind's checker uses TypeScript 7.0.2. Vue, Svelte, and Astro converters live
in `@stillwind/projection`, which uses TypeScript 6.0.3 for operations requiring
the JavaScript SDK. Rust has no dependency on a TypeScript version.

## Compilation path and edit targets

1. The host receives the original document and projects SFCs to TS or TSX as needed.
2. Rust normalizes and scans the projection. It collects macros, type probes, and
   semantic imports from an analysis buffer with inserted receivers such as
   Vue's receiver removed.
3. Public scan results use original-document positions. Declaration identities
   use those positions from their creation. Expressions, producer calls, and
   module specifiers needed by the checker also carry projected ranges validated
   for their AST roles. When a declaration is duplicated in the projection, it
   retains one identity; the checker receives a call range for each copy.
4. The host follows semantic dependencies and prepares required TS, TSX, SFC,
   barrel, and helper inputs. File reads and asynchronous converters finish
   before checker queries begin.
5. A checker-only buffer receives hidden declaration-identity arguments and
   resolved imports. TypeScript reads the framework projection before Rust's
   normalization, preserving framework types for props, loops, and stores.
   JavaScript inputs use JSDoc type assertions.
6. TS7 returns types for the query ranges. The host checks identity diagnostics,
   extracts finite candidates, and passes type facts to Rust's finalize step
   with their original expression ranges.
7. Rust generates macro replacements for the original document and CSS. The
   output remains `.vue`, `.svelte`, or `.astro`, so the framework's ordinary
   compiler processes it.

Steps 1, 2, and 5 create analysis buffers without saving or editing the user's
files. Step 7 produces output edits. Hidden checker arguments and phantom
properties are absent from runtime output.

## Coordinate contracts

| Data                                                                                          | Coordinates                                                      |
| --------------------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| Identities, type probes, type facts, module specifiers, public IR, and diagnostic byte ranges | Original UTF-8 bytes; half-open ranges                           |
| Converter maps, script realms, and edit regions                                               | Original/projected UTF-16 code units; half-open ranges           |
| Native `checkerSpans`                                                                         | Original UTF-8 ranges paired with projected UTF-16 ranges        |
| Native `checkerBindings`                                                                      | Projected UTF-16 ranges of converter-generated helper references |
| Checker AST and shadow edits                                                                  | Checker-buffer UTF-16 ranges                                     |
| Editor diagnostic positions                                                                   | Original UTF-16 offsets and zero-based lines/columns             |

Normalized analysis-buffer ranges exist only during Rust AST extraction. They
are mapped to the original document before publication. Unverifiable mappings
produce projection or coordinate diagnostics. A declaration identity from another
module is not remapped through the use site's source map. Generated symbol
declaration locations also follow the identity.

## Type propagation and errors

TypeScript inference and control flow supply finite candidates for style
composition, refs and scope operands at their use sites, keyframes, and view transitions.

```ts
const a = style("flex");
const b = style("grid");

let selected: typeof a | typeof b = a;
if (flag) {
  selected = b;
}
const result = style(selected, "items-center");

function identity<T>(value: T): T {
  return value;
}
const other = style(identity(flag ? a : b));
```

Declaring `let selected = a` and then assigning `b` makes their hidden identities
incompatible. The checker diagnostic is reported as `SW12001` in original coordinates.
If required candidates cannot be recovered from `any`, `unknown`, `StyleResult<string>`,
or function parameters that erase identity, compilation fails with `SW12002` or a native
unresolved-value diagnostic. Lost type information does not permit selecting a candidate
and reporting success. Ordinary utility conditions use Rust's structural lowering.
Stillwind does not report every TypeScript diagnostic as a general type checker would.

A converter rejecting original syntax produces `SW10002`; failure to parse
converted code produces `SW31004`. Map inconsistencies and missing mappings for
required expressions or declarations produce distinct `SW31001` through
`SW31003` diagnostics. Missing required checker AST produces `SW30004`, which
is distinct from missing type facts.

`scanSource` returns validated `ready` results or `failed` results with diagnostics.
A failed result has no summary, and the host stops dependency traversal and type
queries. Ordinary and collect modes use the same reports. Notes include the
converter's name and version. Labels without a validated original position use
`scope: "document"`. Watch mode retains diagnostics for the failed original
revision and uses the last successful artifacts. A successful update replaces
both diagnostics and artifacts.

`StyleResult` identity uses the readonly, type-only string property
`data-__stillwindStyleIdentity`. It coexists with Svelte's attachment symbol index
signature. Ref, keyframes, and view-transition brands remain symbols. The checker
obtains canonical unique-symbol properties from registered producers' core
modules. Even when type aliases disappear, it extracts finite identities from
brands and checks that they identify registered assets of the same kind.
Refs and keyframes use callable declaration types; view transitions use declaration
types with a callable `for`. Macro-use results and variants are not declaration
candidates. Users who forge identities or deliberately violate the contract through
TypeScript diagnostic suppression are outside these guarantees.

## Framework-specific processing

- Vue: The checker reads Volar's typed projection. Query ranges include the
  `__VLS_ctx.` receiver that Rust removed. Inserted `.value`, unwrap operations,
  shorthand expansion, qualified type names, and constructor parentheses are
  normalized only after validating their original mappings. Generated declarations
  and scopes remain; only import references added for exposure, completion, and
  narrowing are excluded. Normal-script and setup import realms remain separate.
  Generated helper callees use checker-only imports with names that avoid
  projection identifiers. An independent virtual module supplies global helper
  types; user functions with the same names and user `globalThis` bindings cannot
  affect that lookup.
- Svelte: The checker includes svelte2tsx's official shim and JSX declarations
  for props, runes, stores, and each blocks. Generated code is parsed as TS,
  preserving generic arrow functions. Generated `__sveltets_2_*` and
  `__sveltets_$$*` runtime references use the same checker-import mechanism as
  Vue after validation confirms that they have no original reference mapping.
  User references with those names and converter-generated local functions retain
  their bindings. DOM spreads need no adapter that erases identity. A macro call
  written directly in a slot prop fails projection validation because svelte2tsx
  duplicates it for type declarations without a source map. Declare the style
  in the script and pass it to the slot.
- Astro: Frontmatter and templates are TSX. Each original client-script region
  gets an independent checker `.ts` buffer. Client bodies inside converter-generated
  functions become whitespace, preserving positions. Source maps and body
  equality validate the original-script mappings. TSX conversion uses the Go/WASM
  `@astrojs/compiler`. Write client-script generic arrows with a TSX-distinguishable
  form such as `<T,>`. The converter copies `<T>` unchanged into TSX, so Rust's
  projection validation rejects it.

## Dependencies

Converters load lazily. Install only the optional peers for the frameworks used
by the application. `@vue/language-core` supports `>=3.0.2 <4.0.0`; that version
range describes the converter, not the Vue runtime. Global type declarations
returned by a converter enter the checker's virtual `.d.ts` files.

Svelte supports `svelte ^5.0.0` and `svelte2tsx ^0.7.56`.
Astro supports `@astrojs/compiler ^4.0.0`.

| Framework | Optional peers                                                 |
| --------- | -------------------------------------------------------------- |
| Vue       | `@vue/compiler-dom`, `@vue/compiler-sfc`, `@vue/language-core` |
| Svelte    | `svelte`, `svelte2tsx`                                         |
| Astro     | `@astrojs/compiler`                                            |

Vue and Astro runtimes and declarations come from framework packages installed
in the consuming project. The TS6 SDK is a regular projection dependency, and
the TS7 checker is a regular host dependency. They use separate TypeScript instances.

The checker implementation lives in
[type-facts.ts](../packages/host/src/semantics/node/service.ts). Projection and
compilation integration tests live in [sfc-types.test.ts](../packages/host/tests/sfc-types.test.ts).

## Projection and edits

The host selects the converter from the original document format. Rust analyzes
the prepared script buffer with Oxc and edits the original document. Each macro binding
selects its calls' output format independently of direct versus projected input.
An explicit source `framework` must match a component's original format; it does not
override macro outputs.
Framework-specific receiver normalization requires validated provenance from the
converter map; matching a helper's spelling alone is insufficient.

Projection constructors validate source identities, UTF-16/UTF-8 conversions,
realm references, parent graphs, and overlapping edit regions. Parent realms need
not physically contain their children, and template regions can refer to script
realms. Missing expression mappings or unavailable runtime-import placement
produce diagnostics rather than guessed edits. Unedited source slices retain
their original bytes.

The host owns upstream maps. Expression mappings and producer maps use the exact
source-map source identity when composing back to authored documents. Import
insertion follows framework script realms and preserves module directives.
Projected helper buffers and checker shadow edits never become runtime source.
