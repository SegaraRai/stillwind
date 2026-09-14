# Structure-preserving style lowering and the fragment runtime

The JavaScript output selection described here is superseded by
[contextual fragment folding](style-fragment-folding.md). The source evaluation
and CSS ordering contracts remain applicable.

Status: implemented (2026-09-12). It replaces the
flattened conditional lowering described in
[style composition](style-composition.md) and the "rule units and cascade
correctness" section of [compiler design](compiler-design.md).

## Motivation

Conditional utilities are currently flattened into `guard && "class"`
arguments, so every test expression is emitted once per branch. Measured with
the native binding:

```tsx
style("p-4", f() ? "p-2" : "p-1");
// stillwindClsx("_s_83a31043",(f())&&"_s_d42fcee6",!(f())&&"_s_9ccb35be")
```

- `f()` is called twice. With a side-effecting or non-deterministic test the
  runtime can apply both branches at once.
- `await f()`, assignments, `this.x`, and `o.a?.b` tests are duplicated the
  same way. `style(a ? 'p-2' : 'p-1', a ? 'm-2' : 'm-1')` evaluates `a` four
  times.
- Nested conditionals, arrays inside conditionals, and `null` branches are
  rejected with `SHJS002`.
- Guards are evaluated inside the `className` property. An attribute value
  written earlier in the source is therefore evaluated after a later guard:
  `style({ "aria-busy": f() }, b ? "p-2" : "p-1")` emits
  `{className:stillwindClsx(b&&…,!b&&…),"aria-busy":f()}`.

The design below keeps the source expression structure, evaluates each user
expression exactly once, and moves all runtime combination into a small
per-framework runtime.

## Contract

### Structure preservation

Lowering replaces each utility string or utility template literal with the
class name generated for that literal. Every other part of the expression is
emitted verbatim, in place: tests, operators, arrays, member expressions,
optional chaining, `await`, `this`.

```tsx
style("p-4", a && (b ? "p-2" : "p-1"), { "aria-busy": f() });
```

```js
stillwindMerge(
  { [stillwindClassKey]: "_s_base" },
  a && (b ? { [stillwindClassKey]: "_s_p2" } : { [stillwindClassKey]: "_s_p1" }),
  { "aria-busy": f() },
);
```

Class names in this document are illustrative.

### Evaluation semantics

- Every user expression appears exactly once in the output and is evaluated
  exactly once per call, in source order (the evaluation order of the original
  argument list).
- Short-circuiting of `&&` and `?:` is preserved by construction; a nested
  test is only evaluated when its enclosing test allows it.
- A tier that would otherwise evaluate a composite expression twice binds it to
  a temporary with the existing IIFE mechanism. No current tier needs one; see
  the tier rules below.
- Guards keep working in async and class contexts: `await` and `this` stay in
  their original positions.

### Supported conditional syntax

Supported inside a conditional expression, at any depth:

- utility strings and utility templates,
- arrays of the above,
- nested conditionals,
- `null`, `undefined`, and `false` as "no branch",
- attribute and selector objects (see below),
- variant wrappers whose values are any of the above.

Conditional attribute and selector objects become supported as a consequence of
the fragment runtime: `style(a ? { "aria-busy": x } : { "data-y": z })` is
legal, and `style(a ? { "hover:": "bg-x" } : "bg-y")` lowers each wrapper leaf
to its own class.

Still rejected, with the existing diagnostics:

- `||` and `??` between a dynamic value and utility strings (the operands
  would have to be class values, which Stillwind does not accept),
- interpolated conditional variants, for example a utility template whose
  variant segment is chosen at runtime:

  ```tsx
  style(`${a ? "hover" : "focus"}:bg-x`);
  ```

  These are rejected with `SHJS005`.

- `yield` inside a test expression.

### CSS ordering

Unchanged. Unconditional utilities form the base unit and are emitted first;
conditional units follow in source order of their literals; the class attribute
order never affects the cascade; no global interleaver or chunk sorter is
introduced. The position-independent rule "a conditional unit overrides the
base unit" stays as documented, and the new tests must pin it.

The four levels - declaration, class unit, module unit, and delivered CSS - are
described in [CSS ordering](css-ordering.md).

## Fragment runtime

### Fragment shape and symbols

- A fragment stores class contributions under `stillwindClassKey` and the
  style payload under `stillwindStyleKey`. All other keys are ordinary props.
- The keys are symbols exported by each framework runtime module and imported
  by compiled code. Fragments never cross runtime instances (see the
  materialization invariant), so plain `Symbol()` values are sufficient; if
  that invariant is ever relaxed, create them with `Symbol.for` instead.
- Development builds validate that `stillwindMerge` never receives an unknown
  symbol key, which catches a fragment produced by a different runtime copy,
  and that its result contains no symbol keys.

### Helper API

Every framework runtime module (`FrameworkSpec.runtime_import`) exports at
least:

- `stillwindMerge(...values)` — accepts fragments, arrays (flattened
  recursively), and falsy values, in argument order. Class contributions
  concatenate in order; style payloads and all other props are last-wins. It
  returns a fresh materialized object that contains the framework class key and
  the framework style format, and no symbol keys.
- `stillwindToFragment(materialized)` — converts a composition base (or any
  materialized props object) back into a fragment: renames the framework class
  key to `stillwindClassKey`, moves the style payload under
  `stillwindStyleKey`, copies other props unchanged. It is idempotent for
  fragment input.
- `stillwindClassKey`, `stillwindStyleKey` — the symbols above.
- the existing helpers (`stillwindClsx`, handle/asset class helpers, ref and
  animation/transition helpers).

`stillwindMerge` never accepts a materialized object implicitly; bases are
converted explicitly at the call site:

```js
stillwindMerge(stillwindToFragment(base1), stillwindToFragment(base2), { … })
```

### Style payload

- The internal payload is an object of custom properties, as
  `stillwindViewTransitionStyle` already returns.
- Materialization is per framework: object for React, Preact, Solid, and Vue;
  serialized string for Svelte and Astro (custom properties only, `;`
  separated, values through `String()`).
- A payload may already be a serialized string (a materialized base). Merging
  is last-wins and never parses: `stillwindMerge` serializes an object payload
  and passes a string payload through.

### Materialization invariant

The macro call always materializes: the emitted call-site expression is a
helper call (or, for the optimized tiers, a plain object literal), so a
fragment is only ever an argument of that expression. No fragment is bound to a
variable, returned, or exported. This keeps symbol keys out of every
serialization boundary (JSON, structured clone, server/client payloads) and out
of user-visible props. Fixture inspection must fail if a symbol key appears
outside a helper-call argument.

## Emission tiers

Canonical lowering always produces the fragment form. The emitter folds it when
the fold is provably equivalent: identical final props, identical evaluation
order and counts, identical short-circuit behavior.

| Tier | Condition                                                    | Emitted shape                                                                         |
| ---- | ------------------------------------------------------------ | ------------------------------------------------------------------------------------- |
| 0    | No runtime class expression; bases none                      | `{ className: "_a _b", "aria-busy": loading }`                                        |
| 1    | Runtime class expressions exist but no cross-category hazard | `{ "aria-busy": loading, className: stillwindClsx("_a", a && "_b") }`                 |
| 2    | Otherwise, or conditional props, or a style payload          | `stillwindMerge({ [stillwindClassKey]: "_a" }, a && { … }, { "aria-busy": loading })` |

Hazard definition: a runtime non-class expression (attribute value, external
class value, handle/asset expression, view transition value or key) whose source
position falls strictly between the source positions of two runtime class
expressions. Tier 1 evaluates all class expressions at one property, so such an
expression cannot be ordered correctly. `style("p-4", { "data-x": f() }, a && "m-2")`
is still Tier 1 because `"p-4"` is static.

Additional tier rules:

- The object-literal property order is chosen to match source evaluation order.
- Composition bases are always materialized in their own module, so Tier 2
  wraps every base in `stillwindToFragment`.
- Tier 0 and Tier 1 are the only shapes that may omit a runtime import.

The implemented tiers never read a composite expression twice: a composition
base appears once as a merge argument and forces Tier 2, and Tier 0/1 exclude
bases entirely. The IIFE temporary mechanism therefore has no current use; it
returns with any tier that would otherwise evaluate a base or a test twice.

## Metadata and transport

`GeneratedClass.guard` and `guard_when_true` cannot express nesting. Replace
them with an ordered guard path (outermost to innermost test span plus
polarity). The napi output schema changes accordingly and no compatibility
alias is kept. Diagnostics keep attaching to literal spans; unresolved
utilities inside conditionals keep the existing external-class behavior.

## Acceptance

- Compiler tests: single evaluation (side-effect counters), short-circuit,
  source evaluation order including attribute values, `await` and `this`
  tests, nested depth, arrays, `null` branches, variant wrappers, conditional
  attribute objects, and the still-rejected forms.
- Runtime tests per framework: `stillwindMerge` with falsy, array, nested
  array, and fragment inputs; last-wins props; class concatenation order;
  `stillwindToFragment` idempotence and base conversion; object and string
  style payloads; unknown symbol key validation.
- Emission tests: fixture inspection for the materialization invariant, and an
  equivalence test that both tiers produce identical props at runtime.
- Transport: nested conditionals in development and production browser builds,
  SSR/hydration class equality, HMR, and a re-run of `vp run verify:composition`
  inside `experiments/module-graph` for cross-module mixed tiers.
- Ordering regression: emitted CSS keeps base-before-conditional rule order.
- Repository gates: formatter, lint, `cargo test --workspace`, Clippy with
  warnings denied, recursive TypeScript build/typecheck/test/check, and
  `vp run fixtures:refresh` followed by `vp run fixtures:check`.

## Implementation order

1. Per-framework runtime: symbols, `stillwindMerge`, `stillwindToFragment`,
   style materialization, and the `FrameworkSpec` additions (class key already
   exists; add the style format).
2. Extractor and IR: a nested conditional input tree that mirrors the source
   expression, replacing the flat guard pair, plus the guard path metadata.
3. Lowering and emission: structure-preserving lowering, the three tiers, the
   temporary binding for composite expressions, and the napi schema change.
4. Fixtures and documentation: regenerate review fixtures, update
   [style composition](style-composition.md) and
   [compiler design](compiler-design.md) where they describe the flattened
   guards, and link this document from the README next to the composition
   limits.
