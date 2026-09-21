# CSS emission

The applied cascade is the contract. CSS bytes, CSSOM rule indices, and the
physical order of independent declarations are not public API. All transports
preserve the cascade of the [reference order](css-ordering.md).

## Compilation and linking

Compilation produces ordered rule occurrences, their support requirements,
resource references, and dependencies on other producers. These dependencies
come from [style composition](style-composition.md).
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
import { css, parts } from "stillwind/module-css";
import { styleSheets } from "stillwind/css";

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

`styleSheets` returns ordered constructed stylesheets without adopting them.
Equal payloads share a sheet within the same Document. Different Documents use
different caches and constructors. Consumers must treat returned sheets as
immutable, since mutation is visible to every shadow root adopting that sheet.
Each shadow root needs its own ordered adoption list.
