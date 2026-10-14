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

## Materialization and consumption sites

Completed `style(...inputs)` results and declarations from `style.ref(...)`,
`style.keyframes(...)`, and `style.viewTransition(...)` can be stored and passed
independently. Place declarations in binding initializers. Directly nested
`style()` calls are forbidden; compose completed props through variables.

Consume the following syntax directly at its supported input position in `style()`:

- `ref.asAnchor(scope?)`, `ref.asContainer(type?)`, `ref.positionAnchor()`,
  `style.anchorScope(scope)`, `keyframes(options)`, and `transition.for(key?)`
  belong at positions that accept classes or rules. Supported arrays,
  conditional branches, and variant values can contain them.
- `ref()`, `ref("hover:")`, ref tags and their `.in(scope)` calls,
  `ref.at(query?)`, and ref.at tags belong in computed variant-object keys or
  supported utility-template interpolations.

Attribute values and arguments to arbitrary functions cannot consume these uses,
even inside `style()`. Unconsumed keyframes calls produce `SW04002`; other
unconsumed projections and ref variants produce `SW01011`. Validation uses resolved
bindings and checker receiver information. Public use types describe accepted
input syntax; they do not guarantee independently usable runtime values.

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

Conditions, `class`, custom properties, `aria-*` and `data-*` values, composition
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
    "data-value": (classes.active = false),
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

| Macro or input                                                            | Argument contract                                                                                                                                                                                                         |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `style.ref(name?)`                                                        | The name is omitted or any string literal. Whitespace becomes `_`; punctuation is escaped when CSS is emitted.                                                                                                            |
| `style.keyframes(definition)`                                             | A static object. Frames contain utility strings; variant keys contain definition objects that replace the entire definition.                                                                                              |
| `keyframes(options)`                                                      | Options are a string literal. The receiver has finite animation candidates. The use belongs only in a `style()` class input.                                                                                              |
| `style.viewTransition(definition)`                                        | A static object. `group` contains animation options; `old`, `new`, and `imagePair` contain utility string literals. All four buckets accept only media/supports conditional variants. The value is `{v:"<class token>"}`. |
| `transition.for(key?)`                                                    | The receiver has finite transition candidates. Literal keys fold at compile time; expressions run at runtime. Names use `serializeIdentifier(token + "-" + String(key))`.                                                 |
| `ref.asAnchor(scope?)` / `.positionAnchor()` / `style.anchorScope(scope)` | Refs have finite candidates. Receivers are pure selection expressions.                                                                                                                                                    |
| `ref.asContainer(type?)`                                                  | The optional type is a string literal representing a finite union of CSS container-type keywords.                                                                                                                         |
| `ref.at(query?)` / ref.at tags                                            | The receiver has finite raw-ref candidates. The static query returns a variant. An omitted or empty query emits `@container <container-name>` without a condition or parentheses.                                         |
| `ref()` / `ref("variant:")` / ref tags                                    | The prefix is static. Tags cannot contain interpolations.                                                                                                                                                                 |
| `ref(state).in(scope)`                                                    | The scope is a raw ref or an unscoped ref call/tag. Scope operands cannot use `.in()` or `.at()`. Repeated `.in()` calls independently combine containment conditions for the original ref and target with AND.           |
| Utility strings and variant keys in `style()`                             | Static CSS descriptions. Utility templates accept only the specified ref-variant interpolations.                                                                                                                          |
| Arrays and variant objects                                                | Recursively contain supported inputs. Literal spreads and array holes are supported. Getters and methods are forbidden.                                                                                                   |

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
Ref attachments, external `class` values, inline custom properties, attributes,
and style composition cannot occur in variant values. They can occur in other
`style()` arrays and conditional branches. A dynamic view-transition name inside
a CSS variant is stored in a conditional inline custom property; the variant's
rule emits `view-transition-name: var(...)`.

The object keys `""` and `":"` group the same rule inputs without adding variants.
Nested groups retain their enclosing variants. These keys also work in CVA values,
which retain the CVA input restrictions. Attributes, custom properties, and external
classes can be supplied beside the rule groups in a `style()` object.

An incompatible input in a rule group produces `invalid-variant-input`, which
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
(`macro-argument-mutation`) findings. `await`, `yield`, and dynamic imports produce
`MacroExecutionBoundary` (`macro-execution-boundary`) findings. Selectors that
refer to neither an element nor a declaration identity, such as `[:host]:` and
`[::view-transition-old(*)]:`, produce `DocumentScopedSelector`
(`require-element-scope`) findings. Their CSS applies whether the element exists
or not; place document-wide rules in a stylesheet.

These rules default to `deny` (error). CSS `@stillwind` settings can select `warn`
or `off`, and `swlint-disable*` directives can suppress individual findings.
Transform output is retained even when lint findings exist.

A view transition uses a class for identity and a name for its pairing key.
A call without a name supplier produces `MissingViewTransitionName`
(`require-view-transition-name`); a call with two or more produces
`DuplicateViewTransitionName` (`single-view-transition-name`). Both default to
`warn`. They do not inspect name values because other calls, stylesheets, and
user inline styles are outside their view.

If a composition base and the current call declare separate transition classes,
the compiler reports `SplitViewTransitionClasses` (`single-view-transition-class`,
default `warn`). `view-transition-class` is one property: the lists do not merge,
and the derived list replaces the base list. Build hosts fail when findings with
an effective `deny` policy remain.

Suppression does not guarantee preservation of the order or count of effects that
violate this contract. Inputs whose structure cannot be extracted or transformed
produce compilation errors that cannot be suppressed. The compiler does not prove
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
Frame utilities cannot contain variants. Definition conditions accept media and
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
specificity and importance still apply. Bases with additional props or runtime
style payloads are rejected, as are cyclic dependencies. See
[CSS emission](css-emission.md) and [CSS ordering](css-ordering.md) for transport
and output order.

## View transition

```tsx
const transition = style.viewTransition({
  old: "opacity-0",
  new: "opacity-100",
});
<article {...style(transition.for(product.id))} />;
```

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
configuration object directly in the call. Completed-style composition and
declaration uses are not accepted as CVA inputs. See
[runtime](runtime.md#cva-factories) for selection, defaults, compounds, and examples.

## Custom properties

`style()` accepts quoted `--*` fields alongside utility strings, attributes,
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
variable can select the color utility instead.

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
The existing restriction on composing props-bearing style results also applies
to results containing custom properties.

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
serialization supplies one space for true and `initial` for false. A missing
property is false; inherited values follow ordinary CSS inheritance. Strings
such as `"true"`, numeric flags and media queries are not boolean
conditions. Native value comparisons use the separate `--name:value` form below. `@property` registration changes the initial-value contract and must
not be used for these flags.

One leading `!` negates a condition: `--if(!--enabled,20px,10px)` and
`if(!--enabled):flex`. Two or more leading exclamation marks are rejected.
Negated value conditions swap the branches in both strategies. **Negated variants
require `useCssIf: true`**: swapping their branches would store `revert-rule` in
a fallback helper, which cannot preserve that keyword's meaning.

Native mode also supports value comparisons: `if(--mode:compact):flex` and
`--if(--mode:compact,20px,10px)` lower to `style(--mode:compact)` tests. Set
`useCssIf: true`; fallback mode reports a compilation error. Comparison values
must be nonempty, and commas must be inside a CSS function or quoted string.
Matching follows native CSS style-query semantics, including any `@property`
registration. `if(!--mode:compact):flex` negates the comparison; comparisons can
also be combined with boolean conditions using repeated variants.

Repeated variants form an AND: `if(--a):if(--b):flex`. Ordinary variants compose
with them, for example `hover:if(--enabled):flex`. The condition is evaluated on
the styled element (or pseudo-element), not a referenced ancestor. A condition
inside a `ref` wrapper is rejected; put `if(...)` outside that wrapper to test
the target instead.

Value conditionals can nest and appear inside expressions:

```tsx
style("w-[calc(--if(--wide,20,10)*1px)]");
style("bg-[--if(--enabled,red,blue)]");
style("w-[--if(--a,--if(--b,30px,20px),10px)]");
```

Both branches participate in typed utility selection. Use the usual explicit
hint when ambiguous, such as `bg-[image:--if(--enabled,var(--image),none)]`.
Wrap a branch containing top-level commas in braces: `--if(--a,{Arial,sans-serif},serif)`.
Strings, URLs, nested function arguments, and comments keep their lexical boundaries.
Authored declaration values and `@variant if(--enabled) { ... }` use the same
lowering. `@apply grid if(--enabled):flex;` keeps the unconditional declaration
before its conditional rule.

### Output strategy

```ts
stillwind({
  css: "./src/root.css",
  useCssIf: true,
});
```

`useCssIf` defaults to `false`. It is also available in native compiler options;
Rust uses `CompilerOptions::use_css_if`, and the CLI accepts `--use-css-if`.
The option applies to both values and variants and participates in module cache
identity.

| Input                    | Default fallback strategy                                            | `useCssIf: true`                                  |
| ------------------------ | -------------------------------------------------------------------- | ------------------------------------------------- |
| `--if(--flag,20px,10px)` | Local helper `--sw-if-…:var(--flag) 20px;` and `var(--sw-if-…,10px)` | `if(style(--flag):20px;else:10px)`                |
| `if(--flag):flex`        | Same helper pattern, with `revert-rule` as the false branch          | `display:if(style(--flag):flex;else:revert-rule)` |

Helper names are deterministic and honor the configured Stillwind prefix.
They are defined next to their consumers to avoid inheriting a helper value
already computed on another element. Conditions must not depend on the
declaration they guard; normal CSS variable cycle rules still apply.

Fallback mode requires the branch used when the underlying property is true
(the false branch for `!--condition`) to resolve to a valid custom
property token value. An invalid `var()` in that branch can select the false
branch instead. CSS-wide values in that branch (`initial`, `inherit`, `unset`, `revert`,
`revert-layer`, `revert-rule`) require native mode and literal occurrences are
reported as compilation errors. Indirect values that become CSS-wide keywords
have the same limitation. Fallback substitution is not a general replacement
for native `if()`'s branch evaluation semantics.

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
instead, and an absent registration does not make `if(...)` a built-in name in
Rust. Literal prefix and suffix text surround the single unescaped `*`; escaped
asterisks are literal. Ordinary CSS overrides keep their normal registry precedence.
`--if(...)` remains a compiler-defined value function, independent of variant registration.
