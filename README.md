# Stillwind

Stillwind is a CSS-first compiler for component styles. It resolves Tailwind
CSS v4-shaped utilities at build time and emits static CSS, framework props,
and the runtime operations required by conditions and dynamic values.

```tsx
import { style } from "stillwind/react";

<button
  {...style("rounded-md bg-accent px-4 py-2", loading && "opacity-60", {
    "aria-busy": loading,
  })}
>
  Save
</button>;
```

Import `style` from `stillwind/react`, `stillwind/preact`, `stillwind/solid`,
`stillwind/vue`, `stillwind/svelte`, or `stillwind/astro`. Vue, Svelte, and Astro
documents use host-side projection before Rust analysis. Macro argument expressions
must be pure. Evaluation order, evaluation counts, and unused evaluations are not
guaranteed; perform effects outside the macro.

## Configure an application

```ts
// stillwind.config.ts
import { defineConfig } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "./src/stillwind.css",
      files: ["src/**/*.{ts,tsx}"],
    },
  ],
});
```

```css
/* src/stillwind.css */
@import "stillwind";
@theme {
  --color-accent: oklch(0.6 0.2 250);
}
```

```ts
// vite.config.ts
import { stillwind } from "@stillwind/integration/vite";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [stillwind()],
});
```

The application depends on `stillwind` and `@stillwind/integration`.
Configuration discovery supports `.ts`, `.mts`, `.cts`, `.js`, `.mjs`, `.cjs`,
and `.json` in that precedence order within each directory. `configFile` selects
an explicit path; `configFile: false` uses inline settings. Several projects
require an unambiguous selection. See [configuration and analysis](docs/standalone-analysis.md).

The `stillwind` package owns the default theme, preflight, utility slot, and builtin
definitions. A CSS entry without their imports gets no implicit defaults. The
individual exports are `stillwind/theme.css`, `preflight.css`, `utilities.css`,
and `builtins.css`. Custom entries place `@stillwind-slot` markers explicitly.
Hosts resolve imports; Rust consumes supplied CSS and does not embed the package.

Set `classNamespace` to a stable application or package name when independent
builds share a document. Use the same value for client and server builds.
`prefix` and `tailwindPrefix` must start with `--`, end with `-`, contain at least
four bytes with no whitespace, and differ from each other.

## Styles and tooling

```ts
const base = style("m-10 hover:m-20");
const compact = style(base, "m-4");
const progress = style("w-[calc(var(--progress)*100%)]", {
  "--progress": value,
});
```

Composition places base CSS before derived CSS; ordinary specificity and importance
still apply. Ref providers, keyframes, view transitions, CVA factories, custom
properties, and runtime CSS conditions are described in [macro contracts](docs/macro-contract.md).
Rules follow [CSS ordering](docs/css-ordering.md). Build adapters link the completed
producer graph and expose explicit CSS values for Shadow DOM and manual placement;
see [CSS emission](docs/css-emission.md) and [build integrations](packages/integration/README.md).

`stillwind fmt` sorts utility inputs. `stillwind lint` reports project diagnostics,
and `stillwind lint --fix` applies supported fixes. Only changed files are saved.
Use `stillwind fmt && vp fmt` for general formatting, or the
[Prettier plugin](packages/prettier-plugin/README.md) to combine both operations.
The language server exposes sorting and fixes through Code Actions.

## Work in this repository

The pnpm workspace uses Vite+. Tasks live in `vite.config.ts#run.tasks`.

```sh
vp i --frozen-lockfile
vp run verify
vp run --filter @stillwind-internal/apps-play dev
```

Publishable packages use `stillwind` or `@stillwind/*`. Private packages use
`@stillwind-internal/apps-*`, `@stillwind-internal/e2e-*`, and
`@stillwind-internal/tools-*`. The root and benchmark package use
`@stillwind-internal/root` and `@stillwind-internal/benchmarks`.

The static [Playground](apps/play/README.md) runs the compiler and application
builder in workers and previews React in a sandboxed iframe. The
[fixtures viewer](docs/test-inventory.md#review-fixtures) displays representative
generated source, CSS, diagnostics, and maps. Framework acceptance lives under
`e2e/`; explicit performance workloads live under `benchmarks/`.

## Documentation

| Document                                     | Responsibility                                                                         |
| -------------------------------------------- | -------------------------------------------------------------------------------------- |
| [Architecture](docs/crate-architecture.md)   | Crate ownership, service entrypoints, source identity, diagnostics, Node binding       |
| [Macros](docs/macro-contract.md)             | Accepted inputs, purity, composition, refs, animations, transitions, custom properties |
| [CSS ordering](docs/css-ordering.md)         | Declaration, class-unit, module, and delivery precedence                               |
| [CSS emission](docs/css-emission.md)         | Linking, output slots, explicit CSS, comparison, minification                          |
| [Identifiers](docs/generated-identifiers.md) | Identity, naming callbacks, escaping, collision domains                                |
| [Runtime](docs/runtime.md)                   | Lowering, fragments, framework materialization, CVA                                    |
| [Analysis](docs/standalone-analysis.md)      | Configuration, CLI, LSP, lint policy, editor sorting                                   |
| [SFCs](docs/sfc-type-checking.md)            | Projection, checker facts, source coordinates, original-document edits                 |
| [Verification](docs/test-inventory.md)       | Build and test tasks, generation, caches, review fixtures                              |
| [Open work](docs/TODO.md)                    | Unresolved design and validation items                                                 |
