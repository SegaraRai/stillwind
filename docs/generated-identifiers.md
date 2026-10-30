# Generated identifiers

Generated names are allocated from semantic identity and written through types that
normalize and escape names for each CSS or JavaScript position. Consumers must not infer
identity from a prefix or decode a name back into a declaration.

## Identity and naming

Utility classes retain the producer, style call, and branch identity. Ref, keyframes,
and view-transition declarations retain their own declaration identity. Set
`classNamespace` when independently built projects share a document, and use the same
value in client and server builds. Configured property prefixes must start with `--`,
end with `-`, and contain at least four bytes with no whitespace. `prefix` must differ
from `tailwindPrefix` and `themePrefix`, and `themePrefix` may equal `tailwindPrefix`.

`themePrefix` renames theme variables in finished CSS. Resolution, lint, and editor
queries keep authored names, and assembly renames each whole CSS identifier that names
a theme variable or starts with `--default-`, the optional variables of the preflight.
Strings, comments, URLs, and class names keep their text. Generated rules apply
`tailwindPrefix` earlier, so a theme variable that receives the same prefix is not
rewritten by a later `tailwindPrefix`. A reference may spell the prefixed name, which
selects the same variable. With a custom `tailwindPrefix`, a `--tw-` name in an
arbitrary value is rewritten as an internal property first and no longer selects a
variable that `themePrefix: "--tw-"` renames. JavaScript that names a theme variable
is not rewritten. A selected variable whose output name equals a registered property
is an error, because `@blur` tokens such as `--blur` would otherwise replace the value
that their own utility stores in `--tw-blur`.

`Identifiers` supplies a generator for each name family. The generator receives the
family's context and its full hash. Returning the default choice preserves the
compiler's generator. Host-generated names have no automatically added default prefix.
Each compilation memoizes the family, identity, and label, including rejections. Hosts
update `namingRevision` when callback behavior changes so old artifacts cannot reuse
names from the previous generator.

Naming callbacks run synchronously on their owning JavaScript thread. Sessions retain
them because stylesheet rendering also allocates names. Moving such a session across
threads is outside the callback contract.

## Values and escaping

Refs lower to `{ c: token }`; attachments read `.c`, and dynamic projections use
candidate tables keyed by it. Keyframes lower to `{ k: name }`. View transitions lower
to `{ v: token }`, where the token also names `view-transition-class`. Literal
candidates can become static output without a runtime receiver read.

Brand constructors normalize canonical names. Render methods escape those names for
selectors, custom identifiers, properties, or JavaScript strings. Hosts and emitters
must not escape canonical names a second time.

A keyed transition name is the CSS identifier serialization of
`token + "-" + String(key)`. It has no generator or collision pool. Different transition
identities may share a pairing name. A host generator must keep `-` out of transition
tokens so token/key composition stays unambiguous.

## Collision domains

| Pool                    | Members                                                         |
| ----------------------- | --------------------------------------------------------------- |
| Class tokens            | Utility classes and ref markers                                 |
| Anchor names            | `anchor-name`, `position-anchor`, and `anchor-scope` names      |
| Container names         | `container-name` and `@container` names                         |
| Keyframes               | `@keyframes` and animation names                                |
| View-transition classes | Transition identity tokens                                      |
| Custom properties       | Conditional helpers and guarded transition slots                |
| Rule boundaries         | Support-query identities preserving conditional rule boundaries |
| Layers                  | Generated layer names                                           |

Within one pool, a spelling can belong to only one identity. Different pools can share a
spelling. The design system's authored CSS reserves existing names.
`ProjectSession::reserve_authored_css` adds producer-owned CSS to that reservation set.
Names without declaration sites, including anonymous layers and condition helpers, still
participate in document-level checks.

## Development and release

Compiler mode chooses explicit `stillwind/runtime/<output>-development` imports for
runtime validation. Export conditions and `NODE_ENV` do not choose those helpers.
Checked helpers share their framework's production symbol keys.

Release generation failures produce compilation diagnostics. Development mode can retain
default names and records rejected host names for the host to report. Rejected names
have no source location and are separate from author-code lint findings. Every
`StylesheetRenderer` call returns its rejection records beside the CSS, or beside the
fatal cause when rendering fails. A stylesheet plan combines records from its reference
and optimized renders, deduplicating by family, input hash, and reason.
`render_compilation` also retains the module's generation records. N-API and browser
stylesheet outputs expose these records as `rejectedNames`; bundler integrations report
them through their warning channel. Failed N-API renders attach `rejectedNames` to the
thrown error and return no partial CSS.
