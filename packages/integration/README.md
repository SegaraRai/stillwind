# Stillwind build integrations

Stillwind compiles Tailwind CSS v4 utilities into component props and static CSS.
It uses theme rules and compiler checks to keep styling code in order, even when
AI agents write much of it.

`@stillwind/integration` provides build tool integrations for Stillwind. It reads
`stillwind.config.*`, compiles styles during development and production builds,
and delivers generated CSS through the selected build tool.

## Install

```sh
vp i stillwind
vp i -D @stillwind/integration
```

Create the stylesheet and project configuration described in the
[Stillwind package guide](https://github.com/SegaraRai/stillwind/blob/main/packages/core/README.md).

## Vite

Place Stillwind before your framework's JSX transform:

```ts
import { stillwind } from "@stillwind/integration/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [stillwind(), react()],
});
```

Keep your application's other framework plugins. When a framework splits routes
or server functions before JSX compilation, its splitting plugin must run before
Stillwind.

## Other build tools

| Host      | Entrypoint                         | Registration                                                              |
| --------- | ---------------------------------- | ------------------------------------------------------------------------- |
| Astro     | `@stillwind/integration/astro`     | `integrations: [stillwind()]`                                             |
| Nuxt      | `@stillwind/integration/nuxt`      | `modules: [stillwind()]`                                                  |
| SvelteKit | `@stillwind/integration/vite`      | Add `stillwind()` before `sveltekit()`; add the SSR handle from the guide |
| Next.js   | `@stillwind/integration/next`      | `withStillwind(nextConfig)`                                               |
| webpack   | `@stillwind/integration/webpack`   | `plugins: [new StillwindPlugin()]`                                        |
| Rspack    | `@stillwind/integration/rspack`    | `plugins: [new StillwindPlugin()]`                                        |
| Rsbuild   | `@stillwind/integration/rsbuild`   | `plugins: [pluginStillwind()]`                                            |
| Turbopack | `@stillwind/integration/turbopack` | Add `stillwindRules({ entries })` to the host's rules                     |

The [integration guide](https://github.com/SegaraRai/stillwind/blob/main/docs/integrations.md)
contains complete registration examples, SSR setup, plugin ordering, and host limitations.

## Configuration and checks

Use `stillwind()` or the equivalent constructor without options to read the project
configuration. `configFile` selects another file, `project` selects a named project,
and inline settings override the selected project. Several projects must have an
unambiguous selection. See [project configuration](https://github.com/SegaraRai/stillwind/blob/main/docs/standalone-analysis.md#shared-project-configuration).

Build integrations report compiler errors and skip lint checks. Run
`stillwind lint` separately to enforce lint rules.

For split stylesheets, server-only styles, or Shadow DOM, see
[CSS delivery in the guide](https://github.com/SegaraRai/stillwind/blob/main/docs/integrations.md#css-delivery).
