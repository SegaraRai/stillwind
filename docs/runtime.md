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
