# Macro argument contracts

Stillwind macros declare UI. Expressions that compute values must return values
with the same meaning for the same inputs and make no externally observable
changes. This condition applies to **each argument expression inside a macro**.

## Framework entrypoints and shared types

Import `style` from `stillwind/react`, `stillwind/preact`, `stillwind/solid`,
`stillwind/vue`, `stillwind/svelte`, or `stillwind/astro`. These entrypoints live in
`packages/core/src/frameworks/`. React returns `className`; the other frameworks
return `class`. The `style` prop is a string in Svelte and Astro and an object in
React, Preact, Solid, and Vue.

`stillwind/macro` (`packages/core/src/macro/`) owns shared argument types and
declaration identities. `packages/core/src/runtime/` combines compiled props.

Utility literals contribute only candidates resolved by the CSS snapshot and the
[marker classes](css-emission.md#marker-classes) that candidates add as written.
Other candidates are omitted from generated classes, CSS, and canonical class inputs.
The `unknown-utility`, `invalid-utility`, `unknown-variant`, `invalid-variant`, and
`invalid-marker-use` rules report these candidates. Severity settings and suppression
affect reporting without changing omission. Supply external class values through
`{ class: expression }`.

## Unsupported input

Each macro call lowers one to one into a runtime form. Optimizations apply only to the
generated form. Invalid input also has a defined runtime meaning:

- An invalid contribution, such as a utility, option, or entry, is empty.
- An invalid condition, such as a variant, a ref condition, or a media argument, is false.

Omission is the result of this lowering. It is not recovery from an error, so lint
severity and `swlint-*` suppression change reporting without changing output.

The compiler classifies each problem by what it can still determine:

| Problem                                                                                                                                                              | Result                     |
| -------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------- |
| The source cannot be read: parse, source-map, or projection failures                                                                                                 | Compilation error          |
| The runtime form cannot be chosen: missing or invalid checker facts, a macro use outside a supported input position, or inconsistent framework selection             | Compilation error          |
| The runtime form is known and its meaning is empty or false: unknown or invalid utilities and variants, invalid entries and options                                  | Omission and an input lint |
| Output is kept but probably unintended or violates project rules: overlaps, constraints, canonical spelling, `invalid-property-value`, or `document-scoped-selector` | Policy lint only           |
| A constraint outside the one-to-one lowering fails: composition cycles, CSS ordering, generated-name collisions, or missing required slots                           | Compilation error          |
| The environment or configuration is invalid                                                                                                                          | Compilation error          |

CSS ordering of composition bases is an explicit exception to one-to-one lowering,
because a derived call must follow its bases in the stylesheet.

Input lints have the `input` category. The rules for candidates apply wherever a
candidate appears:

- `unknown-utility` reports a utility name or theme value that does not exist, such as
  `unknown-x` or `bg-red-999`.
- `unknown-variant` reports a variant name or theme value that does not exist, such as
  `foo:` or `group-foo:`.
- `invalid-utility` reports malformed candidate syntax, and an existing utility that
  rejects its value form, modifier, or negative sign, such as `p-[calc(1rem+)]`,
  `bg-[url(x.png)]/50`, or `-flex`.
- `invalid-variant` reports an existing variant that rejects its value, its modifier,
  or its inner variant, and arbitrary variant text that is not a valid selector, such
  as `min-[foo]:`, `min-(--x):`, `not-before:`, `group-sm:`, or `[color:red;]:`.
- `missing-document-target` reports a candidate of a document-level style whose
  variants do not select an element, as [Variant targets](#variant-targets) describes.
- `invalid-marker-use` reports a candidate that names a
  [marker class](css-emission.md#marker-classes) but cannot add it, such as
  `hover:group`, `!peer`, or `group/[x]`. It also reports a candidate that resolves to a
  utility of the same name, which keeps its utility output.

Each finding covers the failing part of the candidate, such as the value `red-999` or
the inner variant `sm`. An unknown name gets a spelling suggestion when a close
spelling resolves: `bg-red-999` suggests `bg-red-900`. Valid runtime conditions that
the selected `useCssIf` mode cannot lower use `runtime-condition-requires-css-if`.
Other input lints use `invalid-*` rules. Input lints other than `unknown-utility`,
`unknown-variant`, and `invalid-marker-use` default to `deny`.

The same contract applies to authored stylesheets. A declaration whose `--theme()` call
names an inactive token without a fallback is omitted, and `invalid-theme-reference`
reports the call. [Stylesheet lint](standalone-analysis.md#stylesheet-lint) also checks
definitions: `implicit-nesting-selector` reports variant selectors without `&`,
`duplicate-definition` reports definitions that later definitions hide,
`invalid-theme-value` reports theme token values of the wrong type, and
`ignored-tailwind-feature` reports Tailwind's `@config` and import modifiers, which
loading ignores. `invalid-css-import` and `invalid-css-directive` report the imports and
directives that loading skips. These rules check only stylesheets.

An arbitrary value without a type hint is emitted as written when the grammar of
every matching utility definition rejects it, as in Tailwind: `w-[1/2]` emits
`width: 1/2`, and `text-[12px_foo]` emits `color: 12px foo`. The hazard rule
`invalid-property-value` reports these values and defaults to `deny`. Its severity
does not change the output. A value with a type hint that the utility rejects, such as
`p-[color:red]`, and a value that is not well formed, such as `p-[calc(1rem+)]`, are
invalid utilities and are omitted.

`invalid-property-value` also reports these emitted values:

- A `var()` value without a type hint whose variable has a known type that the
  receiving property rejects. The type is known for an active theme token of a
  namespace with declared types and for a property that an authored `@property` rule
  registers with a simple `syntax`, such as `"<length>"`. `bg-(--text-sm)` sets
  `background-color` to a font size, `w-(--color-red-500)` sets `width` to a color, and
  `bg-(--my-len)` sets `background-color` to a `<length>`. The utility keeps the
  definition that Tailwind selects, and the finding suggests type hints that accept the
  variable. A variable of unknown type is not reported.
- An opacity modifier outside `0%` to `100%`, such as `bg-red-500/150` or
  `bg-red-500/[150%]`. Both emit the percentage unchanged in `color-mix()`, as Tailwind
  4.3.3 does, but `color-mix()` rejects it.
- A negative amount of `opacity()`, `grayscale()`, `sepia()`, or `invert()`, such as
  `grayscale-[-50%]`. Filter Effects forbids it, so the browser discards the whole
  filter.

The finding covers the value, or the modifier after `/`.

Other rules report emitted values and variants without changing the output:

- `clamped-value` (hazard, default `warn`) reports a literal value that browsers clamp
  into its range. `opacity-150` emits `opacity: 150%`, which browsers use as `100%`. The
  rule checks `opacity`, `fill-opacity`, `stroke-opacity`, `flood-opacity`, and
  `stop-opacity` below `0` and above `1`, and the amounts of `opacity()`,
  `grayscale()`, `sepia()`, and `invert()` above `1`, including their backdrop-filter
  forms. These ranges come from specification prose, so the compiler keeps them in a
  curated table.
- `unsatisfiable-variants` (hazard, default `warn`) reports a variant stack whose
  condition can never hold. After `not-*` negation, the condition is an OR of
  alternatives, and the rule reports only when every alternative contains conditions
  that exclude each other: disjoint ranges such as `max-sm:sm`, different values of a
  media feature with one value at a time such as `portrait:landscape`, and a condition
  with its negation such as `hover:not-hover`, `dark:not-dark`, or
  `supports-grid:not-supports-grid`. It compares ranges only in the same unit, compares
  selector conditions only when every selector adds conditions to the styled element,
  and compares other conditions by their text. The finding covers the variants from the
  first to the last contradicting one.
- `redundant-variant` (style, default `warn`) reports a variant that has no effect.
  A duplicate resolves to the same application as an earlier variant, such as
  `hover:hover` or `hover:focus:hover`, when no variant between them selects another
  element. An implied variant compares the same feature in the same unit as a stricter
  variant: `md` implies `sm` in `md:sm` and `sm:md`, and the same holds for `max-*`,
  `@min-*`, and `@max-*`. The finding covers the redundant variant, and its fix removes
  it. This rule is separate from `prefer-canonical-utilities`.
- `prefer-typed-variables` (style, default `off`, enabled by `stylistic`) reports a
  `var()` value without a type hint for a utility whose definitions accept several
  types, such as `text-(--x)`, `bg-(--x)`, `border-(--x)`, `shadow-(--x)`, or
  `text-[var(--x)]`. The finding covers the value, and its fix adds the type hint of the
  definition that receives the value: `text-(--x)` becomes `text-(color:--x)`.

The fixes edit only the utility literal. A variant in an enclosing variant key is
therefore never reported as the redundant variant, although it can make a variant of
the literal redundant. Editors offer each fix as a quick fix, and
`source.fixAll.stillwind` and `lint --fix` apply them.

An invalid CSS variant discards its complete conditioned subtree. Removing only the
condition would apply its rules to elements that the author did not select. An empty
style or CVA value contributes no styles. An empty keyframes or view-transition
definition retains its binding and declaration identity. It emits no definition CSS
and requires no definition slot; valid uses, including `transition.for(key)`, retain
their normal meaning.

An invalid argument of `asAnchor()`, `asContainer()`, `positionAnchor()`, a keyframes
use, or `transition.for()` lowers as if the argument were omitted. An anchor scope entry
that is not a ref contributes no identity.

Invalid CVA values contribute nothing. Unsupported configuration properties and defaults
are omitted independently. Invalid compound conditions discard the complete compound.
An unsupported factory signature becomes an empty factory.

### Conditional receivers

Conditional expressions lower each branch separately. An unsupported branch contributes
nothing or is false, and supported branches keep their meaning. Literal `null`,
`undefined`, and `false` branches select no value without a lint.

A ref condition, a `.in()` scope operand, and a `ref.at()` receiver test every ref that
their branches can supply. A branch that is not a ref contributes no candidate.

A keyframes use, `transition.for()`, and a ref projection read their receiver at runtime.
When a `?:` receiver has an unsupported branch, or the receiver is a `&&` expression,
the use lowers into one use per branch. When every branch is supported, one use selects
its declaration from the receiver value.

A composition operand passes its value to the runtime, which merges only the base's class
in the receiving framework's class prop. A base with inline style keeps its
class and loses the rest. A base from another framework contributes no class and has no
CSS ordering dependency. Both cases report `invalid-style-composition` at the operand,
including when only one candidate base is incompatible.
Style dependency cycles produce `SW11001`; cycles in CSS module placement produce
`SW11002`. Neither cycle can be suppressed because it prevents a valid output order.

### Checker facts

Checker facts carry finite asset candidates or `null` for a confirmed non-asset. Only a
value whose type is primitive is a confirmed non-asset. A value of an object type may be
an asset whose type was widened, so it needs facts. An absent fact or a nonfinite
candidate set does not confirm that a value is unsupported. Required identities must be
established before the compiler can produce output.

## Materialization and consumption sites

Completed `style(...inputs)` results and declarations from `style.ref(...)`,
`style.keyframes(...)`, and `style.viewTransition(...)` can be stored and passed
independently. Place declarations in binding initializers, and compose completed
props through variables.

A call of `style()`, `style.cva()`, `style.ref()`, `style.keyframes()`,
`style.viewTransition()`, `style.media()`, or `style.css()` inside a `style()` or CVA
input is unsupported input. The enclosing lowering would keep the call in JavaScript
without its own lowering. It therefore omits the smallest input that contains the call:
the input itself, the whole conditional for a call in its test, or the object property
for a call in its key or in a `class` or custom-property value. A call
inside a function in the input is also omitted, because the copied source contains it. A
nested `style()` call reports `NestedStyleCall`, and the other macros report
`NestedMacroCall`. Both belong to `invalid-style-input`. Declaration macros accept only
literals, so their own input rules report a nested call.

Consume the following syntax directly at its supported input position in `style()`:

- `ref.asAnchor(scope?)`, `ref.asContainer(type?)`, `ref.positionAnchor()`,
  `style.anchorScope(scope)`, `keyframes(options)`, and `transition.for(key?)`
  belong at positions that accept classes or rules. Supported arrays,
  conditional branches, and variant values can contain them.
- `ref()`, `ref("hover:")`, ref tags and their `.in(scope)` calls,
  `ref.at(query?)`, and ref.at tags belong in computed variant-object keys or
  supported utility-template interpolations.

Arguments to arbitrary functions cannot consume these uses,
even inside `style()`. Unconsumed keyframes calls, projections, and ref variants
produce `SW10004`. Validation uses resolved bindings and checker receiver information.
Public use types describe accepted input syntax; they do not guarantee independently
usable runtime values.

```ts
const transition = style.viewTransition({ old: "opacity-0" });
const props = style(transition.for(key));
const reused = style(props, "block");

const data = {
  transition,
  key,
};
const composed = style({
  "hover:": data.transition.for(data.key),
});
```

Reuse completed props or ordinary data containing a declaration and its arguments.
Do not store a projection such as `const use = transition.for(key)`.

## Runtime value expressions

Conditions, `class`, custom properties, composition
bases, dynamic identity selections, and the receiver and key of `transition.for(key)`
must meet these conditions:

- Inputs remain stable during evaluation. Expressions must not modify data that
  other arguments read.
- Called functions, constructors, getters, proxies, iterators, and value
  conversions obey the same contract.
- Functions may modify newly created temporary data. They must not modify data
  shared with other arguments.
- Do not depend on evaluation order, evaluation count, or execution of expressions
  whose results are unnecessary. `PURE` comments are not required.
- Evaluate `Date.now()`, `Math.random()`, network operations, DOM changes, and
  state updates outside the macro, then pass their results.
- Evaluate `await`, `yield`, and dynamic imports outside the macro, then pass
  their results. Exceptions from omitted expressions are not guaranteed to be observed.
- Function names and class or JSX syntax impose no additional convention. Their
  internal value computations obey the same purity contract.
- Read reactive values in the current computation scope. Do not depend on
  subscriptions unrelated to the macro's value or on execution counts. If needed,
  read the value before the macro within that same scope.

```ts
const props = style(isSelected(item) && "bg-accent", {
  class: buildClasses(state),
  "--width": calculateWidth(layout),
});
```

`buildClasses` may create an array, call `push()` on it, and return it.
Communication between arguments as shown below violates the contract:

```ts
const classes = {
  active: true,
};

style(
  {
    class: classes,
  },
  {
    "--active": (classes.active = false),
  },
);
```

When asynchronous work or state updates are needed, specify their order with
ordinary JavaScript:

```ts
const value = await read();
updateState();

const props = style({
  "--progress": value,
});
```

## Declarations and static syntax

| Macro or input                                                            | Argument contract                                                                                                                                                                                                                        |
| ------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `style.ref(name?)`                                                        | The name is omitted or any string literal. Whitespace becomes `_`; punctuation is escaped when CSS is emitted.                                                                                                                           |
| `style.keyframes(definition)`                                             | A static object. Frames contain utility strings of the `keyframe` registry context; variant keys contain definition objects that replace the entire definition.                                                                          |
| `keyframes(options)`                                                      | Options are a string literal of utilities in the `animation` registry context. The receiver has finite animation candidates. The use belongs only in a `style()` class input.                                                            |
| `style.viewTransition(definition)`                                        | A static object. `group`, `old`, `new`, and `imagePair` contain utility string literals of the `view-transition` registry context. All four buckets accept only media/supports conditional variants. The value is `{v:"<class token>"}`. |
| `transition.for(key?)`                                                    | The receiver has finite transition candidates. Literal keys fold at compile time; expressions run at runtime. Names use `serializeIdentifier(token + "-" + String(key))`.                                                                |
| `ref.asAnchor(scope?)` / `.positionAnchor()` / `style.anchorScope(scope)` | Refs have finite candidates. Receivers are pure selection expressions.                                                                                                                                                                   |
| `ref.asContainer(type?)`                                                  | The optional type is a string literal representing a finite union of CSS container-type keywords.                                                                                                                                        |
| `ref.at(query?)` / ref.at tags                                            | The receiver has finite raw-ref candidates. The static query returns a variant. An omitted or empty query emits `@container <container-name>` without a condition or parentheses.                                                        |
| `ref()` / `ref("variant:")` / ref tags                                    | The prefix is static. Tags cannot contain interpolations.                                                                                                                                                                                |
| `ref(state).in(scope)`                                                    | The scope is a raw ref or an unscoped ref call/tag. Scope operands cannot use `.in()` or `.at()`. Repeated `.in()` calls independently combine containment conditions for the original ref and target with AND.                          |
| `style.media(...variants)`                                                | String literals or interpolation-free template literals of variants whose definitions contain only media conditions. See [Media queries](#media-queries).                                                                                |
| Utility strings and variant keys in `style()`                             | Static CSS descriptions. Utility templates accept only the specified ref-variant interpolations.                                                                                                                                         |
| Arrays and variant objects                                                | Recursively contain supported inputs. Literal spreads and array holes are supported. Getters and methods are forbidden.                                                                                                                  |

A ref declaration creates an independent identity and accepts no parent argument.
`ref(state)` requires a ref element matching the state to be an ancestor of the
target element. `ref(state).in(parent)` requires the parent to contain both the ref
and target as descendants. A `parent("focus")` operand also checks the parent's
focus state. The order of repeated `.in()` calls does not change their meaning.

Selector calls/tags and `.in()` results cannot use `.at()`. Combine container
queries and selector scopes by nesting `style()` keys. Consume calls/tags, scoped
variants, and container queries directly inside `style()`; do not store them in variables.

Variant values accept utilities, ref projections, view-transition uses, keyframes
uses, and arrays, conditionals, or variant objects containing those inputs.
Ref attachments, external `class` values, inline custom properties,
and style composition cannot occur in variant values. They can occur in other
`style()` arrays and conditional branches. A dynamic view-transition name inside
a CSS variant is stored in a conditional inline custom property; the variant's
rule emits `view-transition-name: var(...)`.

The object keys `""` and `":"` group the same rule inputs without adding variants.
Nested groups retain their enclosing variants. These keys also work in CVA values,
which retain the CVA input restrictions. Custom properties and external
classes can be supplied beside the rule groups in a `style()` object.

An incompatible input in a rule group produces `invalid-rule-group-input`, which
defaults to `deny`. The compiler omits the whole property and retains other object
entries. Suppression and severity settings affect reporting; the property is omitted
even when the rule is disabled.

When an identity is known at compile time, reading the receiver at runtime may
be unnecessary. The extra `StillwindDeclarationIdentity` argument in the type
definitions is internal checker information. Users must not use it to execute effects.

Every framework accepts `class` as input. React emits `className`; the others
emit `class`. Svelte and Astro emit a string `style` prop; the others emit an
object. Do not supply a `style` key directly as input.

## Optimization and diagnostics

The compiler uses this contract to reorder argument evaluation, remove empty
branches, expand arrays, and combine adjacent `clsx` transformations. It preserves
CSS precedence, class concatenation order, precedence for duplicate properties,
and the final props' key order. Conditions that carry values retain their selection
structure and remain in their execution scope.

Direct assignments, updates, and `delete` produce `MacroArgumentMutation`
(`mutating-macro-argument`) findings. `await`, `yield`, and dynamic imports produce
`MacroExecutionBoundary` (`suspending-macro-argument`) findings.

A `@custom-variant` selector is a nested style rule selector, in both the shorthand
`(<selector>)` and the block form `{ <selector> { @slot; } }`. As in CSS Nesting and
Tailwind, `& ` is inserted before each branch that contains no `&` or that starts with
a combinator. Such a branch styles another element: `(:hover)` becomes `& :hover`, and
`(> .item)` becomes `& > .item`. These selectors depend on the styled element, so they
do not produce `DocumentScopedSelector` findings. A definition with the
`scope(document)` marker keeps its selectors as written; see
[Variant targets](#variant-targets).

An arbitrary variant selector without `&` tests the styled element, as in Tailwind:
`[:hover]:`, `[.is-open]:`, and `[:hover,:focus]:` become `&:is(...)`. The selector
stays document-wide when `&:is(...)` would be invalid or could never match the styled
element. This happens when the selector contains a pseudo-element, or when the final
compound of a branch contains `:root`, `:scope`, `:host`, `:host()`, or
`:host-context()`. Selectors that stay document-wide, such as `[:host]:`, `[:root]:`,
and `[::view-transition-old(*)]:`, produce `DocumentScopedSelector`
(`document-scoped-selector`) findings. Their CSS applies whether the element exists
or not; place document-wide rules in a stylesheet. When other branches of the list
could test the styled element on their own, the finding names the branch that keeps
the list document-wide: `[:hover,:root]:` names `:root`, and `[:hover]:` styles the
element without it.

These rules default to `deny` (error). CSS `@stillwind` settings can select `warn`
or `off`, and `swlint-disable*` directives can suppress individual findings.
Transform output is retained even when lint findings exist.

A variant selector that uses `&` more than once in one complex selector produces
`RepeatedNestingSelector` (`repeated-nesting-selector`) findings. Examples are `[&+&]:`,
`[&_&]:`, a custom variant `(& ~ &)`, and the inner variant of `group-[&+&]:`,
`peer-[&~&]:`, or `not-[&+&]:`. The check uses a custom variant selector after `&`
insertion, so `(+ &)` reports `& + &`. Each `&` stands for a generated utility class, so
the selector matches by the identity of generated classes. The compiler derives that
identity from the module, the call site, and the contribution branch, so it does not
describe a relationship between elements. Comma-separated alternatives count
separately, including alternatives inside `:is()` and `:where()`. The CSS is still
emitted. Use `style.ref()` to relate elements. This rule defaults to `warn`.

A view transition uses a class for identity and a name for its pairing key.
A call without a name supplier produces `MissingViewTransitionName`
(`missing-view-transition-name`); a call with two or more produces
`DuplicateViewTransitionName` (`duplicate-view-transition-name`). Both default to
`warn`. They do not inspect name values because other calls, stylesheets, and
user inline styles are outside their view.

If a composition base and the current call declare separate transition classes,
the compiler reports `SplitViewTransitionClasses` (`split-view-transition-class`,
default `warn`). `view-transition-class` is one property: the lists do not merge,
and the derived list replaces the base list. Build, development, and watch sessions
skip lint detection and reporting, so `deny` findings never fail a build. Run
`stillwind lint` to enforce the project's lint settings.

Suppression does not guarantee preservation of the order or count of effects that
violate this contract. The compiler does not prove
the behavior of arbitrary function bodies or implementations inside input objects.
An absence of diagnostics does not prove purity; users must satisfy the contract.

The API lives in `packages/core/src/macro/index.ts`, and validation lives in
`crates/js-oxc/src/contract.rs`. See [runtime](runtime.md) for output forms and composition.

## Conditional animation definitions

```ts
style.keyframes({
  from: "opacity-0",
  to: "opacity-100",
  "motion-reduce:": {
    to: "opacity-100",
  },
});
```

Each branch replaces the complete definition; it does not inherit `from` from
the ordinary definition. Nested branches accumulate conditions. When several
conditions match, the later definition in the shared variant order takes effect.
A branch with no retained frame declarations contributes no replacement.
Frame utilities resolve through the `keyframe`
[registry context](css-emission.md#registry-contexts), whose transform and filter
utilities interpolate between frames. Frame utilities cannot contain variants.
Definition conditions accept media and
supports variants that do not require an element selector. Container queries are
rejected because `@container` does not restrict the validity of nested `@keyframes`.

## Ref attachments and providers

```tsx
const menu = style.ref("menu");
const trigger = style.ref("trigger");

<section {...style(menu, menu.asContainer("inline-size"))}>
  <button {...style(trigger, trigger.asAnchor())}>Open</button>
  <div {...style({ [trigger("hover").in(menu)]: "text-accent" })}>Menu</div>
</section>;
```

Only `style(ref)` attaches a marker. `asAnchor` and `asContainer` supply CSS to
the class unit at their use site and apply only to elements carrying that ref's
marker. `positionAnchor` and `style.anchorScope` have no gate requiring a marker.
The relative output order of providers and utilities is not guaranteed. Providers
that simultaneously supply the same property do not combine their values
automatically; detectable conflicts produce lint warnings.

`asAnchor()` supplies only `anchor-name`. Its scope can be omitted, `"all"`,
`"none"`, or a literal array of refs. An array includes the anchor names of every
finite candidate; the receiver itself is included only when explicitly listed.
`style.anchorScope(scope)` supplies only the scope. `asContainer()` supplies
`container-name` and, when its argument is present, `container-type`. Omitted
properties are not reset to their initial values.

Each `.in()` scope must contain both the ref and target elements as descendants.
The scope element itself is excluded from the target. Stateful operands also
check the scope's state. Selector scope and CSS `anchor-scope` are independent.
Container queries use `.at(query?)` on raw refs; combine them with selector scope
by nesting variant keys.

## Style composition

```ts
const base = style("m-10 hover:m-20");
const compact = style(base, "m-4");
const selected = style(flag ? base : compact, "hover:m-30");
```

Composing completed styles places base CSS before derived CSS. Ordinary
specificity and importance still apply. Bases with inline style contribute only their
class and report `invalid-style-composition`, which defaults to `deny`. The public types
reject these bases. Dependency cycles remain
compilation errors because the compiler cannot order their CSS. See
[CSS emission](css-emission.md) and [CSS ordering](css-ordering.md) for transport
and output order.

## Table lookups

```ts
const icon = style({ file: "icon-mdi--file", folder: "icon-mdi--folder" }[kind]);
const stripe = style(["text-red-400", "text-blue-400"][index % 2]);
```

A `style()` input can read one entry of an array or object literal with `table[key]`
or `table.name`. This works wherever the input accepts classes or rules, including
arrays, conditional branches, and variant values. The key selects the entry at
runtime, and the entry contributes its utilities as if it were the input. A key that
names no entry contributes nothing. This includes inherited property names such as
`toString` and array properties such as `length`. A literal key selects its entry at
compile time.

Each entry has its own class unit. Like a conditional branch, it follows the base unit
in [CSS order](css-ordering.md#l3---unit-order-inside-one-module), so an entry
overrides an unconditional utility of the same call regardless of argument position.
One lookup selects at most one entry, so the entries of one table never conflict
with each other.

Array entries use their decimal index as the key, and holes have no entry. Object
keys are lookup keys; they are never read as utilities or variants. Keys can be
identifiers, strings, numbers, or computed literals. A repeated key keeps the
position of its first occurrence and the value of its last.

Table values must be literal utility strings, expression-free templates, `null`,
`undefined`, or `false`. The last three contribute nothing. Literal array and object
spreads are expanded. Other forms omit part of the table, and `invalid-style-input`
reports them:

- A spread of a nonliteral value, a computed key that is not a literal, or the key
  `__proto__` omits the whole lookup.
- Any other value, a getter, or a method omits its entry.

A table that is stored in a variable, returned by a function, or passed to a
function is an ordinary value, not a table lookup.

## View transition

```tsx
const transition = style.viewTransition({
  group: "duration-300 ease-out",
  old: "duration-150",
  new: "delay-150 duration-150",
});
<article {...style(transition.for(product.id))} />;
```

Each bucket styles one pseudo-element through the `view-transition`
[registry context](css-emission.md#registry-contexts). Its timing utilities set the
`animation-*` properties of the browser's default animations, so the new snapshot in
this example fades in after the old snapshot fades out. A utility such as `opacity-0`
in `old` sets the snapshot's own value, which the default fade-out then keeps for the
whole transition.

A declaration has a `view-transition-class` identity. Its name is a pairing key
that matches participants before and after a transition. `.for()` uses an empty
string key. Names for literal keys are emitted in CSS; names for dynamic keys are
assembled at runtime. Dynamic names inside CSS variants use inline custom
properties referenced through `var(...)` in the variant's CSS.

Multiple transitions in one call combine into one class list. Conditional uses
have separate slots, and the list contains every token whose use is active.
Class lists from separate calls replace each other through the cascade. Supply
an explicit name from one source. Different identities can share a name. The
compiler does not validate document-wide pairing or name uniqueness at capture time.

## CVA

`style.cva(base?, config?)` generates CSS for every option at compile time and
creates a factory that returns selected props at runtime. Write its static
configuration object directly in the call. Utility inputs cannot contain JavaScript
`&&` or ternary expressions, including inside arrays and CSS variants. Pass runtime
conditions as variant selections. Completed-style composition and declaration uses
are not accepted as CVA inputs. See
[runtime](runtime.md#cva-factories) for selection, defaults, compounds, and examples.

## Media queries

`style.media(...variants)` returns the media query list of variants as a string, so
JavaScript can test the breakpoints and media variants that the CSS uses:

```ts
const compact = style.media("not-md");
const query = window.matchMedia(style.media("sm:motion-reduce", "print"));
```

The compiler replaces the call with a string literal, such as
`"(width >= 40rem) and (prefers-reduced-motion: reduce), print"`. The string has no
`@media` or braces, and the call contributes no CSS. Store, export, and pass the result
like any string. A change to a variant definition in the CSS changes the string and
transforms the module again.

Each argument is a string literal or a template literal without interpolations. It
contains variants with an optional final `:`. The variants of one argument must all
hold, as in a utility: `sm:motion-reduce` combines both conditions with `and`. The
arguments are alternatives, so the result lists the queries of every argument. A variant
that is itself a media query list, such as a custom variant with two `@media` blocks,
contributes one query per alternative. When one variant supplies the query, its
definition's spelling is kept.

Only variants whose resolved definitions consist of media conditions are accepted.
Variants are checked by their resolved definitions, not by name. `dark` qualifies while
it uses `prefers-color-scheme`, but not after a project redefines it with a selector.
`hover` combines `(hover: hover)` with `:hover`, so it is rejected. The compiler does
not extract the media part, because that part means a device that can hover. Define a
media-only variant for that case:

```css
@custom-variant can-hover (@media (hover: hover));
```

An argument that is empty, is not a literal, or contains a variant that does not
resolve or adds another kind of condition matches no environment. Removing only the
failing variant would match more environments than the author wrote. The other
arguments keep their queries. When no argument can match, the call becomes `"not all"`,
because an empty media query list matches every environment. Unresolved variants report
`unknown-variant` or `invalid-variant`. Other problems, including a call without
arguments, report `invalid-media-query`, which defaults to `deny`. Severity settings and
suppression do not change the string.

A call inside a `style()` or CVA input is omitted with that input, as
[Materialization and consumption sites](#materialization-and-consumption-sites)
describes. Store the result in a binding outside the call.

Editors complete only media variants inside the arguments and preview each argument's
query.

## Stylesheets

`style.css(...inputs, options?)` returns a stylesheet built from utilities, such as for a
shadow root or a scoped style element:

```ts
const sheet = new CSSStyleSheet();
sheet.replaceSync(
  style.css(
    "host:block [:host]:p-4",
    { "[button]:": ["px-2", { "hover:": "bg-red-500" }] },
    { selector: ":host" },
  ),
);
shadowRoot.adoptedStyleSheets = [sheet];
```

The compiler replaces the call with a string literal, and the call adds nothing to the
document stylesheet. Store, export, and pass the result like any string. A change to the
stylesheet transforms the module again.

Inputs are compiled into CSS text, so every input must be static: utility string
literals, interpolation-free template literals, arrays, and variant groups. A group key
that ends in `:` applies its variants to the nested inputs, and the keys `""` and `":"`
add none. `false`, `null`, and `undefined` contribute nothing. Attributes, custom
properties, `class`, refs, conditions, spreads of non-literal values, and other runtime
values are unsupported. They contribute nothing and report `invalid-css-input`, which
defaults to `deny`.

The utilities form one document-level style, as [Variant targets](#variant-targets)
describes, so a variant such as `host:`, `[:host]:`, or `[button]:` selects the element
that a utility styles. A utility whose variants select no element is omitted and reported
as `missing-document-target`. The `selector` option names a default element instead: such
a utility resolves as if it started with `[selector]:`. A utility that selects its own
element keeps it.

The last argument is the options object when it is an object literal whose keys are all
`template` or `selector`. Each value must be a string literal.

- `template` names the `@stillwind-css-template` that places the output. It defaults to
  `default`.
- `selector` is the default element described above.

An unsupported value, or a template name that the stylesheet does not define, reports
`invalid-css-input`, and the option lowers as if it were omitted. Without any template,
the string contains only the rules.

A template defines where the output of a call goes, with the slots of the CSS entry
template:

```css
@stillwind-css-template default {
  :where(:root, :host) {
    @stillwind-slot theme;
    @stillwind-slot property-initials inherited;
  }

  :where(*, :host),
  ::before,
  ::after,
  ::backdrop {
    @stillwind-slot property-initials non-inherited;
  }

  @stillwind-slot keyframes;
  @stillwind-slot utilities;
}
```

The string contains the template's authored CSS, the rules at `utilities`, and the
support that the rules require at the other slots: the theme values and keyframes that
they use together with `@theme static` values, `@property` registrations at
`properties`, and initial values of registered properties at `property-initials`. A slot
that the template omits drops its output, so a template chooses which support it
includes. The string never contains preflight or the project's other rules.

The package's `default` template above, from `stillwind/templates.css`, places theme
values and inherited initial values on the root of the adopting tree, and initial values
of properties that do not inherit on every element. It emits no `@property` rules,
because a registration inside a shadow root has no effect, and its initial values are
unconditional for the same reason. Nothing is layered, so the string joins the cascade
of the tree that adopts it like any other unlayered stylesheet. A later
`@stillwind-css-template` with the same name replaces an earlier one, so a project can
redefine `default` after importing Stillwind, or add templates with other names.

A template must contain a utilities slot and cannot contain definitions such as `@theme`
or `@utility`; either problem is a compilation error. A call inside a `style()` or CVA
input is omitted with that input, as
[Materialization and consumption sites](#materialization-and-consumption-sites)
describes.

Editors complete and preview each input as a document-level style with the call's group
keys and `selector` option. The preview of an input shows only its rules, and the
preview of the whole call shows the string that it lowers to. The `template` option
completes the names of defined templates, and its preview shows the template.

## Custom properties

`style()` accepts quoted `--*` fields alongside utility strings,
external classes, and variant wrappers. Values remain JavaScript expressions:
conditional values retain their short-circuiting. Evaluation is governed by the
macro purity contract above.

```tsx
import { style } from "stillwind/react";

import imageUrl from "./image.png?url";

const props = style("bg-(image:--image) w-[calc(var(--progress)*100%)]", {
  "--image": `url("${imageUrl}")`,
  "--progress": progress,
});
```

The bundler processes the image import, including asset hashing and deployment
base handling. Stillwind does not resolve or rewrite these runtime values.
`bg-(image:--image)` explicitly selects background-image; an untyped background
variable selects background-color instead. See
[Arbitrary values](#arbitrary-values) for the selection rule.

| Value                                      | Inline CSS text                        |
| ------------------------------------------ | -------------------------------------- |
| `"1rem"`                                   | `1rem`                                 |
| `0.5`                                      | `0.5` (no automatic unit)              |
| `true` / `false`                           | One space / `initial`                  |
| `new URL("https://example.com/image.png")` | `url("https://example.com/image.png")` |

Strings are already CSS values. A `?url` import produces a string, so wrap it
in `url(...)` when using it as an image. URL objects receive CSS quoting and
escaping automatically. Supply valid CSS text for the property consuming a string.

```tsx
style(
  {
    "--progress": 0,
    "--size": "1rem",
  },
  active && { "--progress": progress },
);
```

Boolean values act as switches for unregistered custom properties:

```tsx
style({ "--enabled": enabled });
```

```css
.button {
  --enabled-color: var(--enabled) green;
  color: var(--enabled-color, gray);
}
```

True supplies an empty CSS value, leaving `green`; false supplies `initial`,
making the intermediate variable invalid and selecting `gray`. A single space
preserves the empty value through CSSOM, where an empty string would remove the
property. The same switch can be queried with `if(style(--enabled): green; else: gray)`.
Registering the property with `@property` can change its initial-value semantics.
Use `Number(enabled)` when a numeric 1/0 is needed for calculations.

Properties merge independently; a later active entry with the same name wins.
Zero and false are values, not omitted entries. Conditional objects and arrays
work like other style inputs. Keys must be quoted literal names; computed keys
and object spreads retain their existing restrictions. Custom properties cannot
be nested inside CSS variant wrappers: put the variable on the element and let
the variant's utility reference it.

React, Preact, Solid, and Vue receive an object in the returned `style` prop.
Svelte and Astro receive a serialized style string. Custom properties overlay
the generated style payload, preserving an unrelated view transition name.
A composition base containing inline custom properties contributes only its class.

## Runtime CSS conditions

`--if(--condition, yes, no)` chooses a utility value at computed-value time.
`if(--condition):utility` conditionally applies a utility. Neither changes the
element's class list when the condition changes.

```tsx
import { style } from "stillwind/react";

const props = style("grid if(--enabled):flex w-[--if(--enabled,20rem,10rem)]", {
  "--enabled": enabled,
});
```

Boolean conditions are **unregistered custom-property names**. Stillwind's boolean
serialization supplies one space for true and `initial` for false. A missing property is
false; inherited values follow ordinary CSS inheritance. Strings such as `"true"`,
numeric flags and media queries are not boolean conditions. Native value comparisons use
the separate `--name:value` form below. `@property` registration changes the
initial-value contract and must not be used for these flags.

A leading `!` negates a condition: `--if(!--enabled,20px,10px)` and
`if(!--enabled):flex`. Any number of `!` is accepted, and an odd count negates the
condition. The canonical spelling has at most one `!`, and `prefer-canonical-utilities`
suggests it. Negated value conditions swap the branches in both strategies.
**Negated variants require `useCssIf: true`**: swapping their branches would store
`revert-rule` in a fallback helper, which cannot preserve that keyword's meaning.

Native mode also supports value comparisons: `if(--mode:compact):flex` and
`--if(--mode:compact,20px,10px)` lower to `style(--mode:compact)` tests. Set
`useCssIf: true`. Commas in comparison values must be inside a CSS function or quoted
string. An empty comparison such as `--mode:` is the presence test `--mode`, because
boolean conditions hold only a space or `initial`; `prefer-canonical-utilities` suggests
the presence test. Matching follows native CSS style-query semantics, including any
`@property` registration. `if(!--mode:compact):flex` negates the comparison; comparisons
can also be combined with boolean conditions using repeated variants.

Repeated variants form an AND: `if(--a):if(--b):flex`. Ordinary variants compose with
them, for example `hover:if(--enabled):flex`. The condition is evaluated on the styled
element (or pseudo-element), not a referenced ancestor. A condition inside a `ref`
wrapper is rejected; put `if(...)` outside that wrapper to test the target instead.

A candidate with an invalid runtime condition is omitted. Malformed `if()` variant
conditions report `invalid-variant`, and `--if()` functions without three arguments
report `invalid-utility`. With `useCssIf: false`, value comparisons, negated variants,
and CSS-wide keywords stored in fallback helpers report
`runtime-condition-requires-css-if`, which defaults to `deny`. The same rules apply to
the declarations that a utility definition writes, so a candidate whose `@utility` body
has such a condition is omitted too.

In authored CSS, a declaration whose `--if()` cannot be lowered is left out of the
output. `invalid-runtime-conditional` reports it in the stylesheet, and it also reports
a generated declaration that the compiler leaves out for this reason, such as one that
`@apply` adds to a utility.

Value conditionals can nest and appear inside expressions:

```tsx
style("w-[calc(--if(--wide,20,10)*1px)]");
style("bg-[--if(--enabled,red,blue)]");
style("w-[--if(--a,--if(--b,30px,20px),10px)]");
```

Both branches participate in typed utility selection. Use the usual explicit hint when
ambiguous, such as `bg-[image:--if(--enabled,var(--image),none)]`. Wrap a branch
containing top-level commas in braces: `--if(--a,{Arial,sans-serif},serif)`. Strings,
URLs, nested function arguments, and comments keep their lexical boundaries. Authored
declaration values and `@variant if(--enabled) { ... }` use the same lowering.
`@apply grid if(--enabled):flex;` keeps the unconditional declaration before its
conditional rule.

### Output strategy

```ts
stillwind({
  css: "./src/root.css",
  useCssIf: true,
});
```

`useCssIf` defaults to `false`. It is also available in native compiler options; Rust
uses `CompilerOptions::use_css_if`, and the CLI accepts `--use-css-if`. The option
applies to both values and variants and participates in module cache identity.

| Input                    | Default fallback strategy                                            | `useCssIf: true`                                  |
| ------------------------ | -------------------------------------------------------------------- | ------------------------------------------------- |
| `--if(--flag,20px,10px)` | Local helper `--sw-if-…:var(--flag) 20px;` and `var(--sw-if-…,10px)` | `if(style(--flag):20px;else:10px)`                |
| `if(--flag):flex`        | Same helper pattern, with `revert-rule` as the false branch          | `display:if(style(--flag):flex;else:revert-rule)` |

Helper names are deterministic and honor the configured Stillwind prefix. They are
defined next to their consumers to avoid inheriting a helper value already computed on
another element. Conditions must not depend on the declaration they guard; normal CSS
variable cycle rules still apply.

Fallback mode requires the branch used when the underlying property is true (the false
branch for `!--condition`) to resolve to a valid custom property token value. An invalid
`var()` in that branch can select the false branch instead. CSS-wide values in that
branch (`initial`, `inherit`, `unset`, `revert`, `revert-layer`, `revert-rule`) require
native mode. A literal occurrence in a utility omits that utility and reports
`runtime-condition-requires-css-if`. In authored CSS, the declaration is left out and
reported as `invalid-runtime-conditional`. Indirect values that become CSS-wide keywords
have the same limitation. Fallback substitution is not a general replacement for native
`if()`'s branch evaluation semantics.

### Browser and minifier requirements

Conditional **variants require `revert-rule` in both strategies**. Native mode
additionally requires CSS `if()` and its style conditions. This feature is
experimental and is not supported by every browser. The compiler adds a
`@supports (x:revert-rule)` gate around conditional rules; browsers without
that feature retain the unconditional styling.

Conditional rules share a support-query name within each nonconflicting interval
of their owner's emission order: `@supports (x:revert-rule) and (--<hash>:)`.
A property conflict starts a new interval, including shorthand/longhand and
logical/physical overlap. Each `style()` call owns its intervals; authored CSS
uses the outer rule, sharing state across expanded `@apply`, `@variant`, and nested
rules. The default name hashes the owner and interval without a prefix. These
names use a separate `rule-boundary` collision pool and may share spelling with
actual custom properties.

`revert-rule` discards the current rule's declarations for the affected property.
Merging conflicting rules would also discard a declaration the false branch
should fall back to. Custom minifier pipelines must preserve distinct support
queries. Value conditionals alone do not need `revert-rule`
unless a branch explicitly requests it.

Browser acceptance runs with `vp run test:e2e`.

### CSS registration

The package registers the public variant name and its ordering position in CSS:

```css
@stillwind-variant if\(*\) (value: [if-condition]) {
  @stillwind-intrinsic builtin-if-variant;
}
```

`[if-condition]` validates the captured presence test, negation, or value comparison.
The compiler intrinsic lowers it to a computed-value condition. The complete body
must be that intrinsic statement; extra declarations and unknown intrinsics reject.
The capture preserves CSS text, including slashes in comparison values.

Names belong to the CSS registry. The same definition can register `when\(*\)`
instead, and an absent registration does not make `if(...)` a name that
Rust knows. Literal prefix and suffix text surround the single unescaped `*`; escaped
asterisks are literal. Ordinary CSS overrides keep their normal registry precedence.
`--if(...)` remains a compiler-defined value function, independent of variant registration.

## Variant targets

Variants apply from left to right, and the leftmost variant is the outermost one, as in
CSS Nesting: `host:hover:flex` selects `:host:hover`. While variants apply, `&` refers
to the element that is selected so far. This element is the anchor. In `style()`, the
anchor starts as the generated class. In `@apply`, it starts as the selector of the
authored rule. A document-level style, which `style.css()` creates, starts without an
anchor. The first variant that selects an element sets the anchor, and every later
variant applies relative to it.

| Variant                                                     | With an anchor                                        | Without an anchor               |
| ----------------------------------------------------------- | ----------------------------------------------------- | ------------------------------- |
| A selector with `&`, such as `hover:` or `[&_.item]:`       | Relative to the anchor                                | Invalid                         |
| An arbitrary selector without `&`, such as `[:hover]:`      | `&:is(:hover)`                                        | `:hover`, which sets the anchor |
| `[:host(...)]:`, `[:root]:`, and `host:`                    | Document-wide, reported by `document-scoped-selector` | Sets the anchor                 |
| A custom variant without `&`, such as `(:hover)`            | `& :hover`, which styles descendants                  | Invalid                         |
| `not-*`                                                     | A condition on the anchor                             | Invalid                         |
| At-rule variants, such as `sm:`, `dark:`, and `supports-*:` | Unaffected                                            | Unaffected                      |

The package's `host:` variant selects `:host`. A definition declares that its selectors
without `&` select a document-level element. `@stillwind-variant` uses
`scope: document` in its header, and `@custom-variant` uses `scope(document)` after the
name:

```css
@stillwind-variant host (scope: document) {
  :host {
    @slot;
  }
}

@custom-variant shell scope(document) (:host);
```

The marker means that the variant can set the anchor, and it keeps the selector as
written. With it, the variant stays document-wide where an anchor exists and sets the
anchor where none exists. Without it, a selector without `&` is relative to the anchor in
every context, as a nested selector without `&` is relative to its parent in CSS
Nesting: `@custom-variant shell (:host)` means `& :host` and styles `:host` elements
inside the anchor. Such a variant is invalid where no anchor exists. Only variants with
the marker and arbitrary selector variants such as `[:hover]:` set the anchor.
`implicit-nesting-selector` reports a definition selector without `&`, and its fix
writes the `&` without changing the meaning.

A document-level style omits a candidate that does not select an element, such as
`size-full` or `hover:flex`, and `missing-document-target` reports it. The finding
covers the variant that refers to `&` before an earlier variant selects an element, or
the whole candidate when no variant selects one. When a variant with a selector list
sets the anchor, later selectors apply to every branch through `:is()`.
`document-scoped-selector` checks only the variants after the anchor, so
`host:[:root]:flex` reports `[:root]`. [Stylesheet lint](standalone-analysis.md#stylesheet-lint)
applies these rules to `@apply` candidates and `@variant` sequences, whose anchor is the
selector of the enclosing rule.

## Variant negation

`not-*` applies a utility where its inner variant does not apply. The package registers
it with a compiler intrinsic, and the definition must not declare a modifier:

```css
@stillwind-variant not-* (value: [variant], group: not) {
  @stillwind-intrinsic builtin-not-variant;
}
```

The compiler negates the resolved inner variant, so selector variants and media,
support, and container conditions can all be negated: `not-sm`, `not-print`,
`not-supports-[display:grid]`, and `not-@md` each produce the opposite condition. A variant with several rule blocks matches when any block matches, so its
negation requires every negated block. Nested conditions such as the media query and
selector of `hover` are negated as alternatives. Negated selectors use `:not()`, and
`&` inside them refers to the styled element. A negation with more than 32 alternatives
is unresolved.

Variants that contain pseudo-elements, declarations, or `@starting-style` have no
negation, so `not-before`, `not-placeholder`, `not-file`, and `not-starting` are
unresolved. A selector whose `&` is not the final compound of every complex selector
styles another element, so its negation is also unresolved: `not-*`, `not-**`, and
`not-[&_.foo]` are rejected, but `not-[.foo_&]`, `not-group-hover`, and `not-has-checked`
resolve. A custom variant selector without `&`, such as `(:hover)`, styles descendants,
so its negation is unresolved. Tailwind negates it as a condition on the styled element
instead. A document-wide arbitrary selector such as `[:root]` has no negation either.

Stillwind also resolves negations that Tailwind rejects or emits as invalid CSS:
negated negations such as `not-not-hover`, variants with several rule blocks,
media query lists, and combined conditions such as `(a) and (b)`.

A `[variant]` definition without a `modifier:` declaration passes a modifier to its inner
variant. `not-group-hover/menu`, `has-group-hover/menu`, and `in-group-hover/menu`
therefore test the `group/menu` element.

## Arbitrary values

Several utility definitions can share a root, such as the `border-*` definitions for
border-width and border-color. A type hint such as `border-(length:--x)` selects the
definitions that accept that type. An unhinted value selects the definitions that
accept its inferred type: `border-[3px]` sets border-width, and `border-[red]` sets
border-color.

The type of a `var()` reference or a CSS-wide keyword cannot be inferred. Such a
value selects only the earliest registered definition that accepts it, so it never
sets several properties at once. The package's definitions are registered in
Tailwind's order:

| Candidate                             | Property                       |
| ------------------------------------- | ------------------------------ |
| `border-(--x)`, `border-[var(--x)]`   | `border-color`                 |
| `text-(--x)`, `stroke-(--x)`          | `color`, `stroke`              |
| `from-(--x)`, `via-(--x)`, `to-(--x)` | the gradient color stop        |
| `font-(--x)`                          | `font-weight`                  |
| `shadow-(--x)`, `inset-shadow-(--x)`  | the shadow, not its color      |
| `drop-shadow-(--x)`                   | the drop shadow, not its color |

For project `@utility` definitions, the definition authored first wins.

`theme(--name)` in an arbitrary value is replaced with the authored value of the
active theme token at build time: `[--x:theme(--color-red-500)]` sets `--x` to the
color. `theme(--name, fallback)` uses the fallback when the token is not active.
A candidate whose `theme()` call names an inactive token without a fallback, or
names something other than a custom property, is unresolved.

Arbitrary values and modifiers decode as in Tailwind. An `_` becomes a space and `\_`
an underscore, but underscores stay inside `url()` and in the first argument of `var()`
and `theme()`, so `w-[var(--my_width)]` refers to `--my_width`. Other backslash escapes
are kept as written.

Inside a math function such as `calc()` or `min()`, the compiler adds the spaces that
CSS requires around binary operators and after commas, as Tailwind does:
`p-[calc(1rem+2px)]` sets `padding: calc(1rem + 2px)`. An operator that starts an
argument or follows another operator is a sign and stays unchanged.

An arbitrary opacity modifier is a percentage, so `bg-red-500/[50%]` is the same as
`bg-red-500/50`. A percentage outside `0%` to `100%` is emitted unchanged and reported
by `invalid-property-value`, because `color-mix()` rejects it.

An arbitrary gradient value, such as `bg-linear-[to_right]` or `bg-radial-(--x)`, sets
`--tw-gradient-position`. The color stop utilities insert that position before their
stops. Without color stop utilities, the value must be the complete argument list, as
in `bg-linear-[to_right,red,blue]`.

In a variant that takes another variant, such as `group-*` or `has-*`, an arbitrary
selector without `&` tests the element that the outer variant names: `group-[.open]`
applies inside an element that has both `.group` and `.open`. A selector that starts
with a combinator, such as `has-[>img]`, is relative. Only `has-*` accepts it, because
CSS accepts a relative selector only in `:has()`.

Variant values must not contain `var()`, because media queries, container queries,
and selectors cannot evaluate it. `min-(--x)`, `max-[var(--x)]`, and `@min-(--x)`
are therefore unresolved.

A `supports-*` value without `:` that does not start with a function is a property
name. `supports-grid` emits `@supports (grid: var(--tw))`, which is true when the
browser knows the `grid` property. `supports-[display:grid]` and
`supports-[selector(:has(*))]` are used as written.
