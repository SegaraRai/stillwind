# Structural style lowering and the fragment runtime

Lowering keeps class contributions and their guards in a structured tree. Output
selection follows [contextual fragment folding](style-fragment-folding.md), and
source expressions follow the [macro purity contract](macro-contract.md).
The accepted leaf categories and variant boundary are defined in
[the style input design](style-input-design.md).

## Contribution structure

A conditional contributes only its selected arm. Arrays collect contributions in
order; variant wrappers apply to class declarations rather than to runtime props.
Custom properties and ordinary attributes retain their original expression spans.
Lowering does not choose framework property names or style serialization formats.

```tsx
style("p-4", a && (b ? "p-2" : "p-1"), { "aria-busy": computeBusy() });
```

A general representation of this call is:

```js
stillwindMerge(
  { [stillwindClassKey]: "sw_base" },
  a && (b ? { [stillwindClassKey]: "sw_p2" } : { [stillwindClassKey]: "sw_p1" }),
  { "aria-busy": computeBusy() },
);
```

Class names here are illustrative. Folding may replace this with direct props,
combine class helpers, or remove empty branches. Evaluations may be reordered under
the macro contract; their contribution precedence remains unchanged. Expressions
remain in their original execution scope, with mappings to their source occurrences.
Hooks, suspension, and explicit mutations must be evaluated outside macros.

## CSS ordering

Unconditional rules form the base unit and are emitted before conditional
regions. A conditional region collects its rules into one class unit after
walking its children; nested conditions start their own regions. Conditional
units follow the source traversal order of those regions. The order of class
tokens in an attribute does not control the cascade.

The declaration, class-unit, module-unit, and delivered-CSS boundaries are described
in [CSS ordering](css-ordering.md). Folding generated JavaScript does not reorder CSS.

## Fragment representation

Every framework runtime owns three instance-local symbols:

| Key                            | Meaning                                                  |
| ------------------------------ | -------------------------------------------------------- |
| `stillwindClassKey`            | Ordered class contributions                              |
| `stillwindStyleKey`            | A complete style payload; the last non-null payload wins |
| `stillwindCustomPropertiesKey` | Individual custom properties; later values win per name  |

Other keys are ordinary props. The complete style payload and individual custom
properties are distinct channels. Custom properties overlay the materialized style.

`stillwindMerge(...values)` recursively visits arrays and fragments, joins classes
without deduplication, applies ordinary props, and materializes framework output.
Class is inserted first when nonempty, ordinary props follow, and style is applied
at its materialization step. Direct props folding must preserve this key behavior.

`stillwindToFragment(base)` converts an already materialized style result for use
as a composition base. It moves framework class/style props to the corresponding
symbols and retains ordinary enumerable props. An existing fragment from the same
runtime is returned unchanged. Nonobjects are rejected.

```js
stillwindMerge(stillwindToFragment(base), { [stillwindClassKey]: "extra" });
```

React uses `className`; the other frameworks use `class`. React, Preact, Solid, and
Vue receive object style. Svelte and Astro receive a semicolon-separated string.
An existing string payload is passed through rather than parsed.

## Materialization boundary

A completed macro expression always produces framework props, either through the
runtime or directly as an object literal. Compiler-generated symbolic fragments
stay inside that expression. They are not exported or passed across serialization
boundaries. Composition inputs are materialized by their defining macro and
explicitly converted back at the consuming call.

Fragments must use the runtime instance that created their keys. Development
checks reject foreign symbol keys and a merge result containing symbols.

## Metadata and verification

Generated classes retain a guard path with outer-to-inner test spans and polarity.
Expression mappings identify each retained source occurrence independently of the
generated argument order. CSS and JavaScript maps have separate output owners.

Tests compare folded and general materialization across framework runtimes,
including empty values, nested guards, duplicate properties, composition bases,
and object/string style. The frontend tests contract errors before empty-value
elimination. Host and Vite tests verify source mapping after framework projection,
helper imports, and removed imports. Review fixtures provide generated output for
component examples.
