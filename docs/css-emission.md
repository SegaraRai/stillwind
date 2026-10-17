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
slot in the wrong syntactic position is an error, as is a missing slot for a selected
contribution. Repeated slots repeat their selected contributions at every position; the
application owns the resulting cascade across conditions and layers.

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
