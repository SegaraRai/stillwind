# CSS emission

The applied cascade is the contract. CSS bytes, CSSOM rule indices, and the
physical order of independent declarations are not public API. All transports
preserve the cascade of the [reference order](css-ordering.md).

## Compilation and linking

Compilation produces ordered rule occurrences, their support requirements,
resource references, and dependencies on other producers. These dependencies
come from [style composition](macro-contract.md#style-composition).
A content fingerprint identifies shareable bytes. Repeated rules retain their
positions: removing a red rule repeated after a competing blue rule can change
the winning color.

`planStylesheet` includes the selected producers and their transitive dependencies,
rejects missing or stale artifacts, and expands the authored output template.
Observed selections support incremental development. A complete selection must
identify its target and graph revision; the host is responsible for discovering
the complete graph, including lazy modules and cached transforms.

Complete plans optimize generated declarations. Scheduling uses Webref property
interference, including shorthand/longhand and logical/physical relationships.
Fallback declarations retain their relative order. Plain class rules with equal
bodies can share selector lists across intervening independent declarations.
Unsupported selectors, unknown properties, unknown at-rules and conditional
boundaries conservatively prevent movement. In particular, different pseudo
classes do not establish mutual exclusion.

Production Vite output also minifies CSS syntax and whitespace unless
`build.cssMinify` is `false`. Source maps compose through URL rewriting and
minification. Split builds pack adjacent rules with the same producer ownership
into partitions, retaining their original relative positions.

The plan includes a complete `css` string and ordered `parts`. Each
part carries its content key, template slot, occurrence sequence and producer
ownership. Parts carry complete enclosing conditions and named layers. Anonymous
layers and opaque constructs remain atomic. Parts may repeat the same payload
at different positions. Concatenating parts preserves the reference cascade;
its bytes need not equal the optimized complete string.

A stylesheet containing `@namespace` stays in one fragment, preserving its
[stylesheet-local namespace scope](https://www.w3.org/TR/css-namespaces-3/#scope).

## Development

Vite compiles producer updates serially and publishes versioned plans. The client
tracks active producers and their dependencies, and updates one retained
style element in place. Pruned producers stop contributing rules. Module additions,
deletions and dependency changes update the plan without a CSS-specific
full reload. CSS definition edits with unchanged JavaScript do not invalidate
producer JavaScript. Changes to generated JavaScript still use the framework's
normal HMR behavior, which can require a reload.

Native multi-module compilation publishes artifacts only when the entire
transaction succeeds. A failed watch reload retains the previous CSS for display.
Planning rejects retained artifacts that no longer match their analyses.
Definition reload reprocesses retained analyses. Module updates reuse artifacts
whose complete analysis keys have not changed.

## Application builds and SSR

Vite defaults to `emission: "single"`. After graph discovery, the linker emits
one stylesheet for automatic producers and their dependencies. Manual CSS-value
producers do not implicitly contribute document CSS. Transform metadata permits
graph reconstruction, and source modules are revalidated on cached builds.

`emission: "split"` emits reusable CSS fragments and
`stillwind-manifest.json`. The manifest lists occurrences in cascade order and
maps producer identities to the occurrences they need, including common support.
Browser loading inserts links at their manifest positions. Network completion
order does not determine final cascade order. Shared payload files can be used
at several occurrence positions.

SSR uses the same manifest. `stylesheetLinks(manifest, modules)` from
`stillwind/css` returns ordered `{id, href, order}` descriptors. Render their links in
that order and include `data-stillwind-part` and `data-stillwind-order` when the
client will extend the selection. `loadStyles(manifest, modules, document)`
installs missing links in order and resolves when they have loaded. Client and
server must use the same build manifest and identifier configuration.

Webpack and Rspack link their completed compilation graph before rebuilding the
shared CSS module. Their ordinary CSS extraction and HMR integrations consume
that module. Loader-only hosts without a complete-graph hook support explicit
CSS values; automatic document emission reports an integration error.

Independent builds are separate ordering domains. Combining their CSS, adding
unmanaged stylesheets, or manually rearranging links requires an application
placement policy. Emission does not change the cascade rules of external CSS.

## Web Components and explicit values

```ts
import { styleSheets } from "stillwind/css";
import { css, parts } from "stillwind/module-css";

shadowRoot.adoptedStyleSheets = styleSheets(parts, shadowRoot.ownerDocument);
// Alternatively pass `css` to an API that accepts stylesheet text.
```

The compiler rewrites the module-specific import. Both named exports and the
default `css` export are supported. An explicit import suppresses automatic
document loading for that producer. Its plan contains required producer styles
and common support in template order, including common rules that occur after
utility slots. Common payloads, including preflight, are shared through immutable
JavaScript fragment imports.

For a Web Components library, set `automatic: false` on the Vite or bundler adapter.
This also suppresses document loading from shared style modules that do not import
`stillwind/module-css` themselves. Explicit CSS projections remain opt-in document
dependencies. The default is automatic loading for ordinary application modules.

For a stylesheet of a few utilities without preflight or the project's other rules,
`style.css()` returns the string directly; see
[Stylesheets](macro-contract.md#stylesheets).

`styleSheets` returns ordered constructed stylesheets without adopting them.
Equal payloads share a sheet within the same Document. Different Documents use
different caches and constructors. Consumers must treat returned sheets as
immutable, since mutation is visible to every shadow root adopting that sheet.
Each shadow root needs its own ordered adoption list.

## CSS values and output slots

The compiler preserves CSS meaning. Whitespace, color abbreviations, and
constant-expression shortening are not public output contracts. The final asset
pipeline owns minification. Compilation preserves lexical boundaries, declaration
order, empty fallbacks, and selector tokens; it does not repair arbitrary generated
strings to match an oracle's spelling.

`@stillwind-slot theme` and `property-initials` insert declarations inside an
authored style rule. `property-initials inherited` and `property-initials non-inherited`
insert only the initial values of registrations with `inherits: true` and
`inherits: false`, respectively, and `property-initials` inserts both. The stylesheet
supplies their selectors. `utilities`, `keyframes`, and `properties` insert rules. A
slot in the wrong syntactic position is skipped, as
[Skipped statements](#skipped-statements) describes. A missing slot for a selected
contribution is an error. Repeated slots repeat their selected contributions at every
position; the application owns the resulting cascade across conditions and layers.

## Skipped statements

Loading leaves out an invalid statement instead of failing, so the rest of the
stylesheet still compiles. The loader records each skipped statement with its location
in the authored source, and stylesheet lint reports the records of the sources that the
host marks for linting:

- `invalid-css-import` reports an `@import` or `@plugin` statement with an invalid
  specifier, options, or modifiers, an import that closes a cycle, and an import that
  the host cannot resolve or read. The stylesheet loads without that import.
- `invalid-css-directive` reports a Stillwind directive that the loader skips: a slot
  with an unknown name or in a context that cannot contain its output, a directive in
  the wrong block or statement form, an invalid `@stillwind-dynamic` or
  `@stillwind-css-template`, and an invalid `@stillwind-registry` header or statement. A
  definition inside a conditional at-rule, such as `@media`, applies without the
  condition and is reported too.

Both rules default to `deny`. CSS that the loader cannot divide into statements, such as
an unterminated block, still fails the load. The root stylesheet must be readable.

The bundler integrations run without stylesheet lint. They report every skipped
statement as a bundler warning with the rule name and the source location. The Vite
plugin reports them when it loads or reloads the design system. The webpack, Rspack,
and Turbopack integrations report them when they build the document stylesheet.

Generated keyframes and property requirements travel as metadata with CSS
contributions. Renderers do not reconstruct them from serialized CSS.
`StylesheetRenderer::render_mapped_stylesheet` and `ModuleCssArtifact::mapped`
retain that metadata. Renderers with string inputs accept authored CSS; `module_css` and
`to_css` are inspection outputs rather than reconstruction inputs.

## Authored directives

The compiler expands `@apply` and `@variant` in authored CSS before it resolves theme
references. It finds them in the rule tree of the stylesheet, so text in comments and
strings is never a directive. [Stylesheet lint](standalone-analysis.md#stylesheet-lint)
reads the same tree.

`@apply` replaces the statement with the declarations of its candidates. Whitespace and
comments separate candidates. A candidate that does not resolve contributes nothing:
`.btn { @apply unknown-x; }` becomes `.btn {}`.

A candidate with variants becomes nested rules for those variants, and the selector of
the authored rule is the anchor, as [Variant targets](macro-contract.md#variant-targets)
describes. `.btn { @apply hover:flex; }` becomes
`.btn { &:hover { @media (hover: hover) { display: flex; } } }`. As in Tailwind,
candidates without variants come first in utility order. Candidates with variants
follow in variant order, and candidates with the same variants keep utility order.

`@variant` wraps its block in the rules of each comma-separated variant sequence, and
each sequence receives its own copy of the block. A sequence that does not resolve is
a false condition, so it contributes nothing: `.card { @variant nope { color: red; } }`
becomes `.card {}`, and `@variant nope, print { ... }` keeps only the `print` copy.
The leftmost variant of a sequence is the outermost one, so `@variant dark:hover`
puts the `dark` media query around the `:hover` rule. Nested blocks expand inside the
rules of the enclosing sequences. A `@variant`
statement without a block stays in the output unchanged.

## Utility bodies

A `@utility` body can contain nested rules, `@apply`, `@keyframes`, and `@property`, as in
Tailwind.

Variants select the utility element, and the nested selector of the body applies
inside them. `hover:child` with `@utility child { & > span { color: red; } }` selects
`.x:hover > span`.

`@apply` in a utility body expands after the utility's variants are applied. The
applied declarations therefore stay in the rule of their nested selector and under
the variants of the utility.

`@keyframes` and `@property` rules in a utility body move to the `keyframes` slot when a
used utility contains them, because browsers ignore them inside a style rule. A moved
rule keeps its enclosing at-rules, such as `@media`, and loses its enclosing style rules.
An unused utility contributes no such rules, and equal rules from several rules of a
utility are emitted once.

## Utility definitions with the same name

As in Tailwind, every definition of a utility name contributes: a project `@utility`
adds to the package's utility of the same name instead of replacing it. `@utility
container { padding-inline: 1rem; }` keeps the package's `width` and `max-width`
declarations and adds `padding-inline`. The definitions are evaluated in Tailwind's
registration order, `@stillwind-utility` definitions first, then plugin registrations,
then the stylesheet's `@utility` definitions, and each one that resolves is emitted in
that order, so a later definition wins in the cascade. As in Tailwind, the definitions
that `@plugin` statements generate are plugin registrations wherever the statements
appear, so `@utility probe { color: blue; }` overrides a plugin's `probe` before or
after its `@plugin` statement.
Within the `@stillwind-utility` definitions, and within the stylesheet's `@utility`
definitions, matching definitions are
typed branches of one utility, and a candidate selects the branches that accept its
value. When selected branches write declarations for the same selector, the later
definition wins. Rules with different selectors are emitted in registration order, so
`@utility probe { & > p { margin: 1em; } } @utility probe { & > :first-child { margin: 0; } }`
emits the `:first-child` rule last, and it wins for a first paragraph.

A candidate is static when a definition without `*` has its complete name, including a
leading `-` and the modifier. Then only such definitions apply, as Tailwind parses a
candidate with a static utility only as that utility: `@utility shadow-lg { ... }`
replaces the package's `shadow-*` for `shadow-lg`. CSS escapes in a definition name are
decoded, so `@utility tab-1\/2` defines the static utility `tab-1/2`.

A definition only adds to a utility. `@stillwind-undef-utility` removes definitions, as
[Registry contexts](#registry-contexts) describes.

## Registry contexts

Each place where utility strings resolve has its own registry of utility, variant, and
arbitrary-property definitions:

| Registry          | Inputs                                                  | Initial definitions       |
| ----------------- | ------------------------------------------------------- | ------------------------- |
| `default`         | `style()`, `style.cva()`, `style.css()`, and `@apply`   | Statements outside blocks |
| `keyframe`        | The frames of `style.keyframes()`                       | Statements outside blocks |
| `view-transition` | `group`, `old`, `new`, and `imagePair`                  | Statements outside blocks |
| `animation`       | The options of a keyframes use, such as `enter("both")` | None                      |

Statements outside every `@stillwind-registry` block change `default`, `keyframe`, and
`view-transition`. A `@stillwind-registry` block switches the registries that the
statements inside it change, and the registries before the block apply again after it.
A block names one or more registries, separated by commas, and an unknown or missing
name is an error. Blocks may nest, and the innermost block decides. The `animation`
registry changes only through blocks that name it, and it rejects every arbitrary
property until a statement accepts one.

```css
@stillwind-undef-utility shadow-*;

@stillwind-registry animation, view-transition {
  @stillwind-undef-utility duration-*;
  @utility duration-* {
    animation-duration: --value([time]);
  }
}
```

A block contains only these statements, and any other statement is an error:

- `@stillwind-undef-utility <pattern>;` removes the matching utility definitions that
  register before it.
- `@stillwind-undef-variant <pattern>;` removes the matching variant definitions that
  register before it.
- `@stillwind-undef-arbitrary-property <pattern>;` rejects arbitrary-property candidates
  for the matching properties, and `@stillwind-arbitrary-property <pattern>;` accepts
  them again. The last matching statement decides, and a property that no statement
  matches is accepted.
- `@utility`, `@stillwind-utility`, `@custom-variant`, `@variant`, and
  `@stillwind-variant` add definitions in their own layer.
- A nested `@stillwind-registry` block.

Statements register in this order: statements in `@plugin` answers first, then the
other statements, each group in source order. An undefinition therefore removes plugin
definitions wherever the `@plugin` statement stands. A plugin cannot remove
`@stillwind-*` definitions of the package, because its statements register before them.
The definitions that remain resolve in the layer order that
[Utility definitions with the same name](#utility-definitions-with-the-same-name)
describes.

A pattern is `*`, an exact name, or a prefix followed by `*`. `duration-*` matches the
definition `duration-*` and the exact names that start with `duration-`, such as
`duration-initial` and a plugin's `duration-300`. It does not match `-duration-*`,
because a pattern for negative names starts with `-`.

Compilation does not request a candidate of a dynamic family from the host when a
`default` undefinition matches its name.

The package's blocks make these changes:

- `keyframe` writes the transform, filter, and backdrop-filter families through typed
  `--sw-motion-*` registrations, such as `--sw-motion-translate-y` with
  `syntax: "<length-percentage>"`. A browser interpolates a typed registration between
  frames. The `--tw-*` registrations of element utilities have `syntax: "*"`, so a
  frame that set `--tw-translate-y` would switch the value halfway through the
  animation. `@property` cannot register a shadow list, so `shadow-*` and
  `text-shadow-*` write `box-shadow` and `text-shadow` directly, and a frame cannot
  combine a shadow with a ring. The registry removes shadow colors, `ring`, `ring-*`,
  `inset-shadow-*`, `inset-ring`, `inset-ring-*`, `drop-shadow-*`, and the
  `transition`, `duration`, `ease`, `delay`, and `animate` utilities.
- `animation` defines `duration-*`, `ease-*`, `delay-*`, and the fill-mode,
  direction, play-state, and iteration words as `animation-*` declarations, and it
  accepts `animation-*` arbitrary properties.
- `view-transition` replaces `duration-*`, `ease-*`, and `delay-*` with the
  `animation-*` definitions of `animation` and adds the words. These set the animations
  of the pseudo-elements, and the other utilities style the pseudo-elements.

These blocks precede the stylesheet, so they change only the package's and plugin
definitions. A stylesheet `@utility` with a name that a block replaces also reaches
`keyframe` and `view-transition`. For example, `@utility duration-slow` sets
`transition-duration` in a view transition. The `bypassed-undefinition` lint reports
such a definition. Write it in a `@stillwind-registry default` block to keep it out of
the other registries, or in a block that names those registries to define it there.

## Marker classes

A selector that names another class, such as `.card &` or `[class~="not-prose"] *`,
needs that class on some element as written. A marker class is a class that a
candidate adds to the element as written, beside its generated classes, so separately
compiled `style()` calls agree on the class that such a selector matches.

The design system collects marker classes from two sources:

- `@stillwind-marker name;` declares one. `@stillwind-marker name (modifiable);` also
  accepts a named modifier and adds the class as written, so `group/menu` adds
  `group/menu`. The package declares `group` and `peer` this way.
- Every class that the style rule selectors of a `@utility`, `@custom-variant`, or
  `@variant` definition name is an exact marker. This includes classes inside `:not()`
  and `:has()` and whole class attribute matches such as `[class~="name"]`. A utility's
  own name is a marker when its body writes it, as `:where(.prose > :first-child)` does
  in the `prose` utility of `@tailwindcss/typography`. `&` names no class. A partial
  attribute match such as `[class*="name"]` names no class.
- `@stillwind-marker-source "css";` makes the classes that the rules in the string select
  exact markers, as a definition body does. It defines and emits nothing, and directives
  inside the string have no effect.

Definitions from stylesheets, native plugins, and Tailwind plugins contribute in the same
way. `@utility card-title { .card & { color: red; } }` makes `card` a marker, so
`style("card")` adds the class `card`.

A marker name can also be a utility. The candidate then adds the class and contributes the
utility's CSS: with `@utility card { & .card-title { margin: 0; } }`, `style("card")`
adds both `card` and the generated class of `card`.

A marker class is unconditional. A candidate with variants, `!`, a leading `-`, or a
modifier that the marker does not accept adds no class, and neither does a candidate in
`@apply` or in a document-level style. `invalid-marker-use` reports such a candidate.
When a utility of the same name resolves, the candidate still contributes that utility:
`hover:card` applies the `card` utility under `:hover` without the class, as in Tailwind,
and the rule still reports it. `ignoredClasses` lists the marker classes that the rule
does not report.

Editors complete marker class names in ordinary utility inputs whose candidate has no
variants, `!`, or leading `-`. They do not complete them in `@apply` or in document-level
styles. Clients with snippet support also receive `name/` for a modifiable marker. A hover
over a candidate that adds a marker class names the class. It also names the
`@stillwind-marker` statement, the utilities and variants, or the plugin CSS that make
the class a marker.

## Plugins

A plugin is JavaScript that generates Stillwind CSS. A stylesheet loads it with
`@plugin "<name>";`, or with `@plugin "<name>" { option: value; }` to pass options. The
host looks up `<name>` in the `plugins` configuration entry first. Otherwise it imports
the package from the stylesheet and uses its default export. Node hosts use Node
resolution. The browser language server bundles the package from the captured workspace,
as it bundles configurations, so the package must not import Node modules. A
`BrowserProject` without plugin operations accepts only configured plugins.

The options block contains only `name: value;` declarations, and the last semicolon is
optional. The loader rejects other statements, empty values, and values with `{}`
blocks. Comments are not part of a value, and strings keep their quotes, so a `;` inside
a string does not end the declaration.

Node hosts import a package name with Node's module cache, so a changed package takes
effect only in a new process. A relative or absolute path names a local plugin file. Node
hosts bundle that file with its local imports, as they bundle configurations, and register
the bundled files and resolved packages as dependencies of the design system. A change to
a bundled file evaluates the plugin again at the next load. The browser language server
evaluates a package again when the text of any file in its bundle changes.

A plugin reads files through its context: `resolve()` resolves a module request from the
stylesheet with the host's resolution, and `read()` reads a file. Plugins that use these
operations instead of Node APIs also run in browser hosts.

The CSS that the plugin's `css()` returns replaces the statement, at the statement's
position in the graph. That CSS must not contain `@import` or `@plugin`.

A plugin defines a utility family on demand when its CSS contains
`@stillwind-dynamic <prefix>*;`, `@stillwind-dynamic <prefix>[*];`, or both. The first
form accepts named candidates such as `icon-mdi--close`. The second form accepts
bracket candidates such as `icon-[mdi--close]`, whose value is nonempty and contains no
`:`, `[`, `]`, or spaces. When a module uses a candidate of the family that has no
definition, compilation reports the candidate. The host then passes it to the
plugin's `resolve()`, adds the returned CSS after the plugin's CSS, reloads the design
system, and compiles the module again, for at most three rounds. The candidate arrives
without variants, modifiers, or `!`. A plugin can leave a candidate undefined, and the
host does not pass that candidate again while the design system stays loaded. A reload
passes the earlier candidates of the same statement to `resolve()` again, before any
module compiles, so the plugin can read changed files.

A family that accepts both forms resolves a bracket candidate like its named spelling:
`icon-[mdi--close]` is `icon-mdi--close`. Compilation reports the named spelling, and
the `prefer-canonical-utilities` lint suggests it in place of the bracket form. A
family that accepts only the bracket form reports the bracket candidate as written, and
the plugin defines it under that name, such as `@utility icon-[mdi--close]`. The named
spelling then stays undefined.

An ordinary `@utility` definition with the same prefix takes a bracket candidate when it
accepts the bracket value by grammar. Then the family does not receive the candidate, and the
lint keeps the bracket spelling. With `@utility icon-* { --size: --value([length]); }`,
`icon-[2rem]` selects that definition, and `icon-[mdi--close]` still belongs to the family.
The fallback that emits an unhinted arbitrary value as written does not apply to a candidate
of the family. Plugin CSS is an internal source, so source maps and diagnostics do not point
into it.

The `catalog()` of a plugin lists the candidates that editors complete for its families.
The host calls it once with an empty input for the candidates that editors always offer.
It calls it again with the utility text before the cursor, without variants, so a large
family can be listed after the input narrows it. The host keeps only names that start
with the input. A name that ends with `-` is incomplete: editors insert it and request
completion again.
[`@stillwind/plugin-iconify`](../plugins/iconify/README.md) is the official plugin.

Tailwind CSS v4 JavaScript plugins use the same `@plugin` statement. The host accepts
a handler function, an object with `handler` and optional `config`, or a factory from
`plugin.withOptions()`. Factory options accept quoted strings, numbers, booleans, null,
and comma-separated lists. Ordinary handlers reject options.

The adapter supports `addBase`, `addUtilities`, `addComponents`, `matchUtilities`,
`matchComponents`, `addVariant`, `matchVariant`, `theme`, `config`, and `prefix`.
Dynamic utilities accept named and arbitrary values, type hints, negative values,
fractions, and slash modifiers. Callbacks receive values and modifiers as Tailwind
passes them: arbitrary text decoded as in Tailwind, `(type:--name)` as the type hint
and `var(--name)`, and a value from `values` unchanged, so a number stays a number.
An arbitrary value without a type hint reaches a typed registration only when
Tailwind's type inference assigns it one of the types, which is a separate check from
the CSS validation of Stillwind's own utilities. Color utilities apply opacity
modifiers before calling the handler. A `matchVariant` registration accepts a named
value only when `values` maps it to a string, and the bare root uses `DEFAULT`.
The host loads the adapter, and the `tailwindcss` package that it needs, only when a
statement selects a Tailwind plugin or the project sets `tailwind` or `important`, so
`tailwindcss` is an optional peer dependency of `@stillwind/host`.

Callbacks run in the host, and the compiler receives ordinary `@utility` definitions.
A static utility from `addUtilities` keeps its name. As in Tailwind, each selector
registers every class that it contains outside attribute selectors and outside the
arguments of `:not()`, `:nth-child()`, and `:nth-last-child()`: `.outer .inner`
registers `outer` with `& .inner` and `inner` with `.outer &`. Each registration names
the other class, so both are [marker classes](#marker-classes). A top-level `@keyframes`
or `@property` rule of `addUtilities` is global CSS. Such a rule inside a utility, including
one that a `matchUtilities` callback returns, follows [Utility bodies](#utility-bodies). An empty at-rule object, such as `"@apply p-4": {}`,
becomes the statement `@apply p-4;`. The CSS that a `matchUtilities`
callback returns for one candidate becomes `@utility <candidate> (functional) { ... }`,
which belongs to the functional utility of its root, so `duration-300` from
`tailwindcss-animate` adds `animation-duration` to the package's `transition-duration`.
When the plugin registers a `matchUtilities` or `matchVariant` callback, the host runs it
for each value in `values`, except keys that start with `__`, and passes the selectors of
the generated CSS in `@stillwind-marker-source` as empty rules. The classes that its selectors name are therefore markers
before any candidate requests a value, so the class that a candidate adds does not depend
on which module the host compiles first. Two more descriptors reproduce Tailwind's exceptions. A registration whose types
include `any` and another type becomes `(functional, fallback)`, which applies only when
no other definition resolves. When a typed callback rejects an arbitrary value because
of an invalid modifier, `(functional, stop)` ends the evaluation of the definitions that
Tailwind registers later. The output of each `@plugin` statement stays at the
statement's position, and [CSS ordering](css-ordering.md#l1---declaration-order-inside-one-rule)
places its utilities before the other utilities. A stylesheet's `@custom-variant`
replaces a plugin's variant of the same name wherever the `@plugin` statement stands,
as in Tailwind.

Plugin theme configuration supports functions and `extend`, and `theme()` reads the CSS
theme with the same key mapping as Tailwind, such as `colors` to `--color-*` and
`transitionDuration.DEFAULT` to `--default-transition-duration`. Values that the plugins
configure are written back as CSS theme variables. A value that refers to its own
variable, such as daisyUI's `colors.primary: "var(--color-primary)"`, would make that
variable invalid through a cycle, so utilities inline it and no variable is declared.
Ordinary `@theme` values take precedence over them, and they take precedence over
`@theme default` values. Tailwind's v3 default theme answers `theme()` reads without CSS
or plugin values, but it is never written back.

A configured `fontFamily.sans` also sets `--default-font-family`,
`--default-font-feature-settings`, and `--default-font-variation-settings`, which the
preflight uses for documents. A configured `fontFamily.mono` sets the matching
`--default-mono-font-*` variables for code. Settings that a `[family, settings]` tuple
omits become `normal`. These variables are written even without the default theme.
When an ordinary `@theme` value defines `--font-sans` or `--font-mono`, the
corresponding variables are not written, as in Tailwind.

All configurations, including presets and the configurations of nested plugins, merge
in registration order. The project's `tailwind` field takes the place of Tailwind's
`@config` file. The loader removes a stylesheet's `@config` statement, and the
`ignored-tailwind-feature` lint rule reports it. The field accepts a configuration or a
promise for one. Its configuration merges after every plugin configuration, so its keys
replace the same keys of plugin configurations, as in Tailwind. The project's `important`
field sets the `important` key in the same way, and setting it together with
`tailwind.important` is an error.

The theme configuration, `important`, `darkMode`, `theme.container`, and the
registrations of the plugins in the `tailwind` field apply to the whole stylesheet.
The native loader therefore adds a `@plugin` request after the root stylesheet, and the
host answers it with their CSS. `config()` reads Tailwind's resolved configuration: `theme`,
`darkMode`, `prefix`, `blocklist`, and `important`, where the last configuration that
defines a key sets it. Other keys return the fallback, as in Tailwind. Paths use
Tailwind's key syntax, so `config("theme[colors][brand]")` and
`config("theme.colors.brand")` read the same value.

`important: true` makes every declaration of a class utility `!important`, including
the package's utilities. A selector string, such as `important: "#app"`, nests every
class utility rule in a rule with that selector, so `p-4` matches only inside `#app`.
Neither form changes `@apply`, and keyframes stay at the stylesheet root.
`theme.container` adds a `container` definition after the package's one, as Tailwind
does: `center` adds `margin-inline: auto`, `padding` adds `padding-inline` for the
default and for each breakpoint, and `screens` replaces the maximum widths.

`darkMode` redefines the `dark` variant with Tailwind's mapping. `"selector"` becomes
`&:where(.dark, .dark *)`, `"class"` becomes `&:is(.dark *)`, and a `[mode, selector]`
tuple replaces `.dark`. `"media"` uses `prefers-color-scheme`, and
`["variant", selectors]` uses the selectors, each of which must contain `&`. As in
Tailwind, this variant replaces a plugin's `dark` variant, and a stylesheet's
`@custom-variant dark` replaces it.

`config()` returns `prefix` and `blocklist`, but these keys do not change the output.
Generated class names are unique without `prefix`, and blocked candidates still generate
CSS. The `no-restricted-utilities` lint rule replaces `blocklist`.

Editor catalogs list finite candidates without running their callbacks. Selecting a
completion resolves its CSS without adding that candidate to the application's used CSS.
CSS distributions load through `@import`. Their `--spacing(<value>)` becomes
`calc(<spacing scale> * <value>)` with the theme's spacing scale, and
`--alpha(<color> / <alpha>)` becomes a `color-mix()` as in Tailwind. The scale function
comes from the registry statement `@stillwind-theme-namespace --spacing-* [length] scale;`,
which makes `--spacing` the scale of the `spacing` namespace. Any stylesheet can declare
another namespace with a scale in the same way. Tailwind multiplies every plain number in
`--alpha()` by 100, so `--alpha(red / 50)` is 5000%, which `color-mix()` rejects; write
`50%` or `0.5`.

`scale(px: 1px)` also declares fixed captures. Both `--value(--spacing())` and
`--value(--spacing-*)` try named tokens before fixed captures and numeric multiples.
Fixed captures require an active scale token and participate in completion and
canonical replacements. A later namespace declaration replaces its types and scale.
`@stillwind-theme-exclude --font-* --font-weight-*;` excludes a narrower namespace
from the wider namespace's `--value()` patterns.

Plugin selectors that name other classes, such as the component classes of daisyUI and
the `not-prose` class of `@tailwindcss/typography`, match through
[marker classes](#marker-classes). Browser hosts require plugin packages that can execute
without Node APIs.

The projects in [`e2e/plugins/tailwind`](../e2e/plugins/tailwind/stillwind.config.ts)
exercise installed plugins in development and production. The package's
[comparison cases](../e2e/plugins/tailwind/tests/comparison.test.ts) build each plugin with
Tailwind and with Stillwind and compare the computed styles in Chrome. Run them with
`vp run --filter @stillwind-internal/e2e-tailwind-plugins test:e2e`, or select
`tailwind-plugins` in the E2E matrix.

The projects in [`e2e/plugins/iconify`](../e2e/plugins/iconify/stillwind.config.ts) check
that the package's sizes override icon sizes in development and production. Run them with
`vp run --filter @stillwind-internal/e2e-iconify-plugin test:e2e`, or select `iconify-plugin`
in the E2E matrix.

## Browser targets

Set `target` in `stillwind.config.ts` to minimum browser versions:

```ts
import { defineConfig } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "src/stillwind.css",
      files: ["src/**/*.{ts,tsx}"],
      target: ["chrome111", "firefox113", "safari17"],
    },
  ],
});
```

`target` accepts one string or an array. Browser names are `android`, `chrome`,
`edge`, `firefox`, `ie`, `ios`, `opera`, `safari`, and `samsung`. Versions contain
one to three decimal components, such as `safari16.2`. Omission or an empty array
keeps color compatibility output.

Targets only remove generated color fallbacks and their capability queries.
They do not lower CSS syntax or change authored `@supports` rules. Every target
must support a value before its fallback is removed. `color-mix()` requires
Chrome/Edge/Android 111, Firefox 113, Safari/iOS 16.2, Opera 97, or Samsung 22.
Colors containing `currentcolor` or runtime color variables require Safari/iOS 17,
matching the preset's guard against earlier WebKit failures.

Without removal, color compatibility output follows Tailwind: known theme values
can supply an sRGB fallback, unresolved variables and `currentcolor` keep the
first color without its opacity, and fixed arbitrary colors need no extra branch.
Final asset minification can evaluate a fixed fallback for older browsers.

## Tailwind comparison

`tools/tailwind-compat/comparison.ts` compares CSS under an explicit shared
environment. It reserves `properties, theme, base, components, utilities` order,
places Tailwind's theme keyframes into Stillwind's theme slot for comparison,
removes `source(none)` when candidates are supplied explicitly, and resolves only
recorded resource placeholders against the fixture's virtual base.
Inside support conditions, equal-valued `background` and `background-image`
tests are equivalent for this comparison; ordinary declarations have no such exception.

The comparison retains layer bodies and conditions, property registrations,
fallbacks, importance, selectors, keyframe contents, and interfering declaration
order. CSS parsing, calc evaluation, and Lightning CSS normalize representation.
Lightning CSS lowers nesting on both sides before minification.
Empty custom-property values remain distinct from missing or invalid values.
These rules define the harness context; they do not prove arbitrary CSS equivalence.
Integration tests separately check real resource owners, URLs, maps, and delivery order.

## Final asset minification

`stillwind-css` provides final asset minification through Lightning CSS with the
`minify` Cargo feature. The operation is independent of compilation and project
selection. Call it after expanding Stillwind directives, selecting the asset's
rules, and resolving resource URLs.

```rust
use stillwind_css::{CssMinifyOptions, minify_css};

let result = minify_css(css, &CssMinifyOptions {
    filename: "app.css".into(),
    source_map: true,
    ..CssMinifyOptions::default()
})?;
```

The Node binding exposes the same service:

```ts
import { minifyCss } from "@stillwind/native";

const result = minifyCss(css, {
  filename: "app.css",
  targets: {
    chrome: 120 << 16,
    safari: (17 << 16) | (2 << 8),
  },
  sourceMap: true,
  inputSourceMap: upstreamMapJson,
});
// result.css, result.sourceMap (JSON string when requested), result.warnings
```

The existing bundler integrations choose their own minification path. Exporting
this function does not automatically apply it to compiler output.

### Options

| Rust field                | JavaScript field        | Default and contract                                              |
| ------------------------- | ----------------------- | ----------------------------------------------------------------- |
| `filename`                | `filename`              | `input.css`; input identity for diagnostics and maps              |
| `targets`                 | `targets`               | Unspecified; browser names mapped to packed versions              |
| `include`                 | `include`               | Empty; features to always transform                               |
| `exclude`                 | `exclude`               | Empty; features to preserve even for older targets                |
| `source_map`              | `sourceMap`             | `false`; return a separate v3 map                                 |
| `input_source_map`        | `inputSourceMap`        | Absent; v3 JSON mapping this exact input CSS to its sources       |
| `include_sources_content` | `includeSourcesContent` | `true`; include source text in the output map                     |
| `unused_symbols`          | `unusedSymbols`         | Empty; names the caller has established are unused                |
| `error_recovery`          | `errorRecovery`         | `false`; allow invalid input to be discarded with parser warnings |

Browser keys are `android`, `chrome`, `edge`, `firefox`, `ie`, `ios_saf`, `opera`,
`safari`, and `samsung`. Versions are `major << 16 | minor << 8 | patch`, with one
byte per component. Configuration lookup and Browserslist resolution belong to the
host. Unknown browser keys and values larger than 24 bits are errors.

Features use Lightning CSS's case-sensitive names: `Nesting`, `NotSelectorList`,
`DirSelector`, `LangSelectorList`, `IsSelector`, `TextDecorationThicknessPercent`,
`MediaIntervalSyntax`, `MediaRangeSyntax`, `CustomMediaQueries`, `ClampFunction`,
`ColorFunction`, `OklabColors`, `LabColors`, `P3Colors`, `HexAlphaColors`,
`SpaceSeparatedColorNotation`, `FontFamilySystemUi`, `DoublePositionGradients`,
`VendorPrefixes`, `LogicalProperties`, and `LightDark`. The groups `Selectors`,
`MediaQueries`, and `Colors` are also accepted. Unknown names and overlapping
include/exclude flags are errors, including overlap through group flags.

### Source maps and errors

An input map requires output mapping to be enabled. Ordinary v3 maps are accepted;
indexed maps are rejected. Input map `sourceRoot` is retained on the composed map.
Embedded `sourceMappingURL` comments are ignored. The host publishes the returned
map and attaches its URL to the CSS if needed. Neither CSS imports nor map URLs
are fetched by this API.

Mapping granularity, coordinate behavior, and CSS optimization follow Lightning
CSS and Parcel Source Map. The wrapper does not repair upstream map coordinates.
Merged rules can have coarser source attribution than the compiler's original
range mappings. The returned map is debugging metadata, not an exact edit map.

Rust errors distinguish options, parsing, minification, printing, and source-map
processing. NAPI converts them to JavaScript errors. Parser warnings are returned
as strings with locations when available. Recovery does not suppress failures in
later processing stages.
