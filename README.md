# Stillwind

Stillwind is a CSS-first, static CSS-in-JS compiler for component code. It uses
Tailwind CSS v4-shaped theme variables and cascade layers, but turns each
import-bound `style()` call into a compact generated class and a framework prop
object at build time.

```tsx
import { style } from "stillwind/react"

<button
  {...style(
    "inline-flex items-center rounded-md bg-accent px-4 py-2 text-white hover:bg-accent/90",
    loading && "opacity-60 cursor-wait",
    { "aria-busy": loading },
  )}
/>
```

The React/Vite example is in [`examples/react`](./examples/react). It compiles
the call above into static classes, keeps only the conditional class join at
runtime, and emits CSS like:

```css
@layer theme, base, components, utilities;
@layer theme { :root { --color-accent: oklch(.6 .2 250); } }
@layer utilities {
  ._s_x { background-color: var(--color-accent); }
  ._s_x:hover { background-color: color-mix(in oklab, var(--color-accent) 90%, transparent); }
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

`@stillwind/native` is a thin loader. Production installs select exactly one
optional package for the host, such as `@stillwind/native-win32-x64-msvc` or
`@stillwind/native-linux-x64-gnu`; generated target packages contain the
platform-specific `.node` artifact. The local build task writes to the same
target-package layout, so development exercises the production resolution path.
`vp run --filter @stillwind/native sync-platform-packages` refreshes package
metadata after the target list changes; `prepare-publish` is the release-stage
task that assembles every CI-built artifact and pins optional package versions.

## CSS-first theme compatibility

Stillwind accepts a Tailwind v4-style `@theme` block. The current compiler indexes
`--color-*`, `--spacing-*`, `--radius-*`, `--shadow-*`, `--text-*`, and
`--breakpoint-*` custom properties and only resolves utilities backed by those
tokens. It currently supports a tested compatibility slice of layout, flex,
spacing, colors, radii, shadows, opacity, transitions, responsive breakpoints,
state pseudo-classes, `aria-[name=value]`, and `data-[name=value]` variants.

That makes incremental migration straightforward: retain the existing `@theme`
surface, then replace selected `className` values with `style()` calls. It does
not yet claim the entire Tailwind utility/variant grammar, nor the planned
Stillwind handles, selectors, local animations, Vue, Svelte, or formatter/linter.
Those need their own specification fixtures before implementation.

Tailwind v4 documents this CSS-first model, its `@theme` namespaces, and its
layer order in its [theme documentation](https://tailwindcss.com/docs/theme)
and [functions/directives reference](https://tailwindcss.com/docs/functions-and-directives).
