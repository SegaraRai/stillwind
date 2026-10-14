# Generated runtime imports

The compiler imports each framework's runtime from `stillwind/runtime/react`,
`stillwind/runtime/preact`, `stillwind/runtime/solid`, `stillwind/runtime/vue`,
`stillwind/runtime/svelte`, or `stillwind/runtime/astro`. Authored macro imports remain
`stillwind/react`, `stillwind/vue`, and the corresponding framework entrypoints.

Each runtime exports ordinary functions and independent symbol keys. Framework wrappers
call shared functions for merging fragments, converting element props, and serializing styles.
Framework conventions and symbol keys are passed as a readonly tuple. Its labels exist only in
TypeScript, so configuration field names do not remain in minified JavaScript.
Bundlers can remove unused operations and symbol keys. JavaScript modules have no required
import side effects; CSS files retain their side effects.

Development compilations explicitly import checked helpers from
`stillwind/runtime/<framework>-development`. These modules share symbol keys with the
corresponding production runtime. They validate fragment ownership, class contributions,
and materialized props before merging or converting them. Dynamic ref attachments and projections that read `.c` call
`stillwindRefClassChecked(ref)` to validate `.c`; release compilations read `ref.c` directly.
The generated code evaluates the ref expression once in either mode.

The framework development modules import their complete `options` tuple from the production module.
They provide checked class concatenation, ref reads, keyed view transition names, and CVA selection.
`stillwindCvaChecked` checks external class inputs and selected fragments each time the factory is called.
Checked class concatenation accepts strings, numbers, bigints, nested arrays, and objects
whose truthy values enable their keys. Booleans, null, and undefined add no class text.
Functions and symbols are rejected. A checked ref must supply a nonempty string in `.c`. Checked view transitions require a nonempty
string token or a record whose `.v` contains that token.

The compiler mode selects these imports. Export conditions and `NODE_ENV` do not select runtime
validation. Production fragment operations assume the compiler's contracts and omit development
checks. Fragment merging keeps contribution order, later authored props and style payloads replace
earlier values, and generated custom properties are applied last. Each framework owns its keys;
fragments must be materialized by the runtime that created them.

## CVA factories

`style.cva(base?, config?)` compiles utility values and creates a selector using
`stillwindCva` from the framework runtime. The configuration keeps its JavaScript
structure. Each base, option, and compound style value becomes compiled class text
or private fragments. Calls to the returned factory remain ordinary JavaScript.

```tsx
const button = style.cva("inline-flex rounded-md", {
  variants: {
    size: {
      small: "px-2 py-1",
      large: "px-4 py-2",
    },
    disabled: {
      false: null,
      true: "opacity-50",
    },
  },
  defaultVariants: {
    size: "small",
    disabled: false,
  },
  compoundVariants: [
    {
      size: ["small", "large"],
      disabled: false,
      class: "hover:shadow-md",
    },
  ],
});

<button {...button({ size: "large" })} />;
```

Import `VariantProps` from the framework entrypoint to extract the factory's
selection props. The type excludes the external `class` and `className` inputs.
Boolean options accept booleans. Undefined selections keep defaults, and null
suppresses the selected axis. Every matching compound contributes in array order.
The factory combines base, selected options in `Object.keys()` order, matching
compounds, and external classes. Variant selections are not forwarded to the element.

Write configuration objects and condition arrays directly in the call. Their keys
must be unique plain static properties; spreads and computed configuration keys are
rejected. The class prop names are reserved axis names. Defaults and compound
conditions must name declared axes and literal options.

Style values accept literal utility strings, expression-free templates, arrays,
boolean/null/undefined values, conjunctions, ternaries, ordinary CSS variant objects,
and root custom properties. Attributes, external-class objects, stored input values,
declaration uses, and style composition are unsupported. CSS variants accept only
rule inputs. Dynamic utility construction is rejected.

Custom properties always remain inline, including literals, so later selected
values replace earlier values without conflicting with CSS extraction. Dynamic
values are captured when the factory is created and obey the ordinary macro purity
contract. Factory calls select captured values without re-evaluating their expressions.
Each call returns fresh framework class and optional style props without fragment
symbols. CVA results cannot be passed to `style()` for composition.

All options contribute CSS at compilation. The factory performs no utility merging
at runtime. Generated rules follow Stillwind's CSS ordering; competing utilities
from separate values are not guaranteed to reproduce Tailwind's global utility order.
Additional `class` and `className` inputs are external class values and generate no CSS.

## Lowering and materialization

`StyleInput` retains arrays, JavaScript conditionals, props, composition, and rule
inputs. CSS variants contain only `RuleExpression` children. Attributes, external
classes, ref attachments, custom properties, and composition cannot occur below
a CSS variant. Conditional branches distinguish conjunctions from ternaries.

Lowering resolves the rules in each JavaScript condition region before allocating
its class. Rules without their own runtime identity share that class. A nested
condition starts a new region. Provider rules require their ref marker through
`:where(...)`; only ref attachments add the marker. Finite dynamic declarations
generate every candidate's CSS and select its class through the receiver value.
Singleton candidates can fold to static output.

Fragments carry three framework-owned symbol channels:

| Symbol                         | Value                                                   |
| ------------------------------ | ------------------------------------------------------- |
| `stillwindClassKey`            | Ordered class contributions                             |
| `stillwindStyleKey`            | A whole style payload; the last non-null payload wins   |
| `stillwindCustomPropertiesKey` | Individual custom properties; later values win per name |

Other keys are ordinary props. `stillwindMerge` recursively visits arrays and
fragments, joins class text without deduplication, and materializes framework props.
Custom properties overlay the whole style payload. `stillwindToFragment` converts a
materialized composition base to a fragment that holds only the base's class. A base
contributes the receiving framework's class prop; its attributes, style, custom
properties, and another framework's class prop are ignored. The development variant
rejects nonobjects.
An existing string style payload is passed through without parsing.

The completed macro always returns framework props. Symbolic fragments remain
inside that macro expression and belong to the runtime instance that created
their keys. They cannot cross serialization or framework runtime boundaries.

The fragment compiler chooses direct props, class expressions, or a general merge
under the [macro purity contract](macro-contract.md). It removes empty branches
and flattens compiler-owned sequences when the result preserves class order,
property overwrite precedence, and materialized key order. It keeps opaque
payloads and value-bearing conditionals as boundaries. CSS variant conditions
stay in CSS; JavaScript guards retain their execution scope and short-circuiting.

Retained expression mappings identify each source occurrence independently.
Moving an argument changes its generated position while preserving its original
span. Generated syntax and helpers have synthetic mappings; removed expressions
have no output mapping. Fragment selection does not change CSS order or identity.
