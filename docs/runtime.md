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

The framework development modules provide checked class concatenation, ref reads, and keyed view transition names.
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
    size: { small: "px-2 py-1", large: "px-4 py-2" },
    disabled: { false: null, true: "opacity-50" },
  },
  defaultVariants: { size: "small", disabled: false },
  compoundVariants: [{ size: ["small", "large"], disabled: false, class: "hover:shadow-md" }],
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
