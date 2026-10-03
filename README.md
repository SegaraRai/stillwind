# Stillwind

Stillwind is a CSS-first, static CSS-in-JS compiler for component code. It uses
Tailwind CSS v4-shaped theme variables and cascade layers, but turns each
import-bound `style()` call into a compact generated class and a framework prop
object at build time.

```tsx
import { style } from "stillwind/react";

<button
  {...style(
    "inline-flex items-center rounded-md bg-accent px-4 py-2 text-white hover:bg-accent/90",
    loading && "opacity-60 cursor-wait",
    { "aria-busy": loading },
  )}
/>;
```

The React/Vite example is in [`examples/react`](./examples/react). It compiles
the call above into static classes, keeps only the conditional class join at
runtime, and emits CSS like:

```css
@layer theme, base, components, utilities;
@layer theme {
  :root {
    --color-accent: oklch(0.6 0.2 250);
  }
}
@layer utilities {
  ._s_x {
    background-color: var(--color-accent);
  }
  ._s_x:hover {
    background-color: color-mix(in oklab, var(--color-accent) 90%, transparent);
  }
}
```

Inline custom properties accept JavaScript values without generating per-value CSS:

```tsx
style("w-[calc(var(--progress)*100%)]", { "--progress": progress });
```

See [custom properties](docs/custom-properties.md) for URL values, bundler-owned
image imports, conditional values, and framework output formats.

Macro argument expressions must be pure: ordinary calculation calls are allowed,
but shared mutation and effects belong outside the macro. Evaluation order and
unused evaluations are not guaranteed. See [macro contracts](docs/macro-contract.md)
for static arguments, reactive reads, execution boundaries, and diagnostics.

See [experimental runtime CSS conditions](docs/runtime-if.md) for `--if(...)`,
`if(...):utility`, and the shared `useCssIf` compiler option.

## Workspace

This repository contains a pnpm workspace backed by Vite+.

```powershell
vp i --frozen-lockfile
vp run verify
vp run --filter @stillwind/react-example dev
vp run --filter @stillwind/play dev
```

Workspace packages declare their Vite+ tasks under `vite.config.ts#run.tasks`; use `vp run`
instead of package scripts. `verify` combines static checks and behavior tests,
including generated-data and project-fixture comparisons. See the
[task and test inventory](docs/test-inventory.md) for generation, environment
checks, caching, and toolchain requirements. `@stillwind/native` is a napi-rs Node-API addon, so
the Vite plugin calls Rust directly rather than spawning `cargo` for every
transform. The interactive playground lives in [`apps/play`](./apps/play): it
owns its compile API and React preview bundler in `apps/play/server`, using the
host CSS loader and an independent native session for each request. Its Vite
configuration enables this app-specific server plugin only during development;
the reusable Stillwind integration serves CSS and HMR. Play displays the transformed
source, CSS, diagnostics, and a live CSS preview. Play accepts TSX components
that export `App` (named, default, or the shorthand `export App;`), recompiles
them as you type, and mounts them in an opaque-origin iframe with scripts only.
The preview response applies a CSP that blocks network connections, forms,
navigation, external images, and popups; user code never executes in the host
Play page.

Compiler changes can be reviewed as durable before/after examples in the
internal [`apps/fixtures-viewer`](./apps/fixtures-viewer) app. Its static catalog
is generated from schema-versioned projects under [`fixtures/projects`](./fixtures/projects),
including multi-module cases. See the [review fixture workflow](./docs/review-fixtures.md)
for permalink format, snapshot generation commands, and the rule for linking a
representative transform from PRs and issues.

Stillwind-owned CSS identifiers and Tailwind-compatible internal properties use
separate configurable namespaces:

```ts
stillwind({
  theme: "./src/stillwind.css",
  prefix: "--sw-",
  tailwindPrefix: "--tw-",
});
```

Both prefixes must start with `--`, end with `-`, and differ from each other.

Every rule-bearing `style()` call has its own class identity, including the
project-relative producer path. Set `classNamespace` to a stable
package/application name when independently built projects share a document; use
the same namespace for client and server builds. Content deduplication and the
former `classIdentity` option have been removed to preserve composition order.

Compose existing results without rewriting their utility strings:

```tsx
const base = style("m-10 hover:m-20");
const compact = style(base, "m-4");
const selected = style(flag ? base : compact, "hover:m-30");
```

Each base's CSS precedes the derived CSS; normal CSS specificity and importance
still apply. Bases with additional props and dependency cycles are errors.
See [composition support and transport limits](docs/style-composition.md).
Conditional and nested utilities keep their source structure: each expression is
evaluated once, in source order, and the per-framework fragment runtime combines
the generated classes. See
[structure-preserving style lowering](docs/style-fragment-lowering.md).
The four ordering levels - declaration, class unit, module unit, and delivered
CSS - and the cascade rules that go with them are described in
[CSS ordering](docs/css-ordering.md).

Vite imports document CSS once, then transports each producer's CSS and support
dependencies through a self-contained CSS module. Late transforms and dynamic
imports do not depend on a previously loaded aggregate stylesheet. This is
module-level CSS retention; it does not eliminate individual unused style calls.

`@stillwind/native` uses the ESM loader generated by napi-rs. The loader first
tries the host's native addon, then falls back to WASI when native loading fails.
Platform packages such as `@stillwind/native-win32-x64-msvc` contain the `.node`
artifact. Native compilation writes cacheable artifacts to
`packages/native/.build/native`; the `build` task installs changed files into
`packages/native/dist` and `binding.d.ts`.
Run `vp run --filter @stillwind/native generate:platform-packages` when changing
the target matrix or updating the napi-rs CLI's runtime dependencies. It generates
platform package metadata; it does not compile or collect artifacts. Ordinary
native builds use the metadata already in `packages/native/npm`.

The same API can be built as a WASM package using the
[NAPI-RS WASI target](https://napi.rs/docs/concepts/webassembly):

```sh
rustup target add wasm32-wasip1-threads
vp run --filter @stillwind/native build:wasm
vp run test:wasm
```

The WASM build writes to `packages/native/dist/wasm`, separately from the native
loader and declaration file. `test:wasm` and `test:browser` collect the built
local targets into their platform packages under `packages/native/npm` before testing.
Import `@stillwind/native` for native-to-WASI fallback. Set
`NAPI_RS_FORCE_WASI=error` to require WASI without loading the native addon.
The root package's browser entry selects the WASI package and its generated
browser loader and workers. Browser hosts must serve
`Cross-Origin-Opener-Policy: same-origin` and
`Cross-Origin-Embedder-Policy: require-corp` for shared WebAssembly memory.
`test:wasm` runs the shared binding contracts through the root loader, tests
fallback with packed packages, checks production bundling, and runs the browser
entry with Vitest Browser Mode in Chrome.

Before releasing, build every configured target and place its artifacts and the
generated native loaders in `packages/native/dist`, and place the WASM loader
set in `packages/native/dist/wasm`. Run
`vp run --filter @stillwind/native collect:release` to assemble all platform
packages. The CLI rejects an incomplete target set. Run `pnpm exec napi pre-publish` from
`packages/native` with `--root-publisher pnpm --npm-dir npm --tag-style npm` in
the release job to validate, synchronize versions, and publish platform packages.
This command publishes packages; it does not build or collect artifacts, and it
does not publish the root package. Use its `--dry-run` flag for validation without
publishing, then publish the root package with pnpm after the platform release.

## CSS-first theme compatibility

Import the CSS definitions explicitly in the entry configured by `css`:

```css
@import "stillwind";
@theme {
  --color-accent: oklch(0.6 0.2 250);
}
```

The `stillwind` package contains the canonical CSS files; Rust does not embed
them. Its `theme.css`, `preflight.css`, `utilities.css`, and `builtins.css`
exports can also be selected individually. A stylesheet without those imports
receives no default utilities, theme, or reset. The utilities export contains
`@stillwind-slot utilities;`; a custom entry can place that marker directly.
Generated rules retain the marker's layer, supports, and media wrappers.
Compile-time definitions must be unconditional; conditions belong around the
output slot or ordinary CSS. Unsupported import modifiers produce diagnostics.
Low-level APIs accept fully supplied CSS; hosts resolve every import through
`NativeDesignSystemLoader` or `DesignSystemLoader`, including package exports.

Stillwind accepts a Tailwind v4-style `@theme` block. The compiler indexes
`--color-*`, `--spacing-*`, `--radius-*`, `--shadow-*`, `--text-*`,
`--duration-*`, `--ease-*`, `--z-index-*`, and `--breakpoint-*` custom
properties and only resolves token-backed utilities. It supports a tested slice
of layout, flex/grid, sizing, spacing, colors, outlines, transforms, radii,
shadows, transitions, responsive breakpoints, state pseudo-classes,
`aria-[name=value]`, and `data-[name=value]` variants.

That makes incremental migration straightforward: retain the existing `@theme`
surface, then replace selected `className` values with `style()` calls. The
implemented Stillwind-specific surface includes finite conditional branches,
element selectors, ref-tagged parent selectors, scope-aware relationships,
anchor/container projections, local utility animations, and view-transition
assets. The compiler checks all supported utility
spellings; Vite also writes `src/stillwind.generated.d.ts` with the theme-backed
token vocabulary for editor completion.

Tailwind's `container` / `@container`, `group`, and `peer` spellings are accepted
for migration compatibility, including named markers and responsive container
variants. Stillwind's `style.ref()` declares independent element identities, and
`ref("hover").in(parent)` constrains a selector at its use site.
Projects that want to enforce that model can set
`forbidTailwindRelationships: true` in the Vite plugin options; compatibility
syntax then emits the `prefer-typed-relationships` lint finding.

Ref provider checks match marker attachments within the same `style()` call.
Calls that contain composition suppress missing-attachment warnings because a
base may supply the marker. Vue, Svelte,
Astro, a full formatter, and full Tailwind candidate compatibility remain
separate frontend/grammar work.

### CSS entry points

The default entry point preserves the usual one-import setup. It includes the
built-in theme, preflight, generated-utility layer, and Stillwind's built-in
utility, variant, and custom-property registry:

```css
@import "stillwind";
```

Applications that assemble those pieces individually must opt into the registry
explicitly:

```css
@import "stillwind/theme.css" layer(theme);
@import "stillwind/preflight.css" layer(base);
@import "stillwind/utilities.css" layer(utilities);
@import "stillwind/builtins.css";
```

Omitting `stillwind/builtins.css` disables built-in utilities, variants, and
their tree-shakeable custom-property registrations. Authored `@theme`,
`@utility`, `@variant`, and `@stillwind-property` definitions remain available.
All four entry points are ordinary package CSS files; the compiler consumes the
recognized imports while preserving unrelated CSS imports.
Their canonical sources live in `packages/core/css` and are included in the
`stillwind` package; exported CSS never imports files outside the package.
`tailwindcss` imports are ordinary host-resolved imports, not aliases for Stillwind.
The current loader preserves ordinary CSS import `layer`, `supports`, and media
conditions. Unsupported build-time modifiers such as `prefix(...)` produce an
error. Conditional imports of the legacy Stillwind entry selectors remain
unsupported until those selectors are replaced by resolved CSS definitions.

### Extending the same registry

Built-ins and application extensions use the same CSS registry. `@utility`
supports Tailwind-shaped `--value(...)` domains, while `@variant` uses `@slot`
for the generated selector. A trailing `*` captures the candidate suffix.
Arbitrary values use Tailwind-shaped CSS value types such as `[length:…]`,
`[color:…]`, and `[grid-track-list:…]`. Stillwind never exposes a
property-name escape hatch: built-ins use standard value domains where
possible, and narrowly named value domains only for CSS grammars that cannot
be reduced to one.

```css
@utility surface-* {
  background-color: --value(--color-*);
}

@utility panel-width-* {
  width: --value([length]);
}

@variant state-* {
  &[data-state="--value()"] {
    @slot;
  }
}

@variant narrow-* {
  @media (width < --value(--breakpoint-*)) {
    @slot;
  }
}

@stillwind-property --panel-progress {
  syntax: "<number>";
  inherits: false;
  initial-value: 0;
}
```

The examples above compile `surface-brand`, `state-open:bg-accent`, and
`narrow-sm:grid` without a JavaScript registry entry. In selectors, `--value()`
substitutes the wildcard suffix; in conditional at-rules, token domains resolve
to the theme token's concrete value. An attribute-name capture uses CSS's
escaped-star spelling, for example `[aria-\*="true"]` in `@variant aria-*`.
`@stillwind-property` declares a tree-shakeable registration: Stillwind emits
the corresponding standard `@property` only when the custom property is used
by the combined theme, user, or generated CSS. Ordinary authored `@property`
rules remain untouched and take precedence over a compiler-owned definition
with the same name.

Tailwind v4 documents this CSS-first model, its `@theme` namespaces, and its
layer order in its [theme documentation](https://tailwindcss.com/docs/theme)
and [functions/directives reference](https://tailwindcss.com/docs/functions-and-directives).

Browser framework acceptance lives in [e2e/frameworks](./e2e/frameworks/README.md) and runs with `vp run test:e2e`. Synthetic performance measurements live in [benchmarks](./benchmarks/README.md).
