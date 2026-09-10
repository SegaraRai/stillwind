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

## Workspace

This repository contains a pnpm workspace backed by Vite+.

```powershell
vp i
vp run --filter @stillwind/native build
vp run --filter @stillwind/react-example dev
vp run --filter @stillwind/play dev
```

Each app declares its Vite+ tasks under `vite.config.ts#run.tasks`; use `vp run`
instead of package scripts. `@stillwind/native` is a napi-rs Node-API addon, so
the Vite plugin calls Rust directly rather than spawning `cargo` for every
transform. The interactive playground lives in [`apps/play`](./apps/play): it
calls that binding through the Vite dev server and displays the transformed
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
for permalink format, snapshot refresh commands, and the rule for linking a
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

The Vite adapter defaults to `classIdentity: "module"`. It salts generated style
classes with the project-relative producer path. Set `classNamespace` to a stable
package/application name when independently built projects share a document; use
the same namespace for client and server builds. `classIdentity: "content"` keeps
content-based names (the default of the lower-level Rust API).

Vite imports document CSS once, then transports each producer's CSS and support
dependencies through a self-contained CSS module. Late transforms and dynamic
imports do not depend on a previously loaded aggregate stylesheet. This is
module-level CSS retention; it does not eliminate individual unused style calls.

`@stillwind/native` is a thin loader. Production installs select exactly one
optional package for the host, such as `@stillwind/native-win32-x64-msvc` or
`@stillwind/native-linux-x64-gnu`; generated target packages contain the
platform-specific `.node` artifact. The local build task writes to the same
target-package layout, so development exercises the production resolution path.
`vp run --filter @stillwind/native sync-platform-packages` refreshes package
metadata after the target list changes; `prepare-publish` is the release-stage
task that assembles every CI-built artifact and pins optional package versions.

## CSS-first theme compatibility

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
variants. Stillwind's `style.ref()` and `style.scope()` remain the safer native
model because their relationships are local and can be checked structurally.
Projects that want to enforce that model can set
`forbidTailwindRelationships: true` in the Vite plugin options; compatibility
syntax then emits the blocking `SHLINT001` diagnostic.

The JSX linter checks ref selectors, scope ancestry, and container providers whenever
both sides are visible in the same JSX tree. It deliberately does not infer
relationships across component boundaries, `children`, or portals. Vue, Svelte,
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
