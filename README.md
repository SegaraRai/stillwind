# Stillwind

Stillwind compiles Tailwind CSS v4 utilities into component props and static CSS.
It is designed to keep styling code in order, even when AI agents write much of it.

With Stillwind, you can:

- require colors, spacing, and other values to come from your theme;
- check that styles depending on a parent or sibling element actually refer to that element;
- define animations and view transitions inside the component that uses them, without adding global names.

```tsx
import { style } from "stillwind/react";

<button {...style("rounded-md bg-blue-600 px-4 py-2 text-white", loading && "opacity-60")}>
  Save
</button>;
```

Stillwind supports React, Preact, Solid, Vue, Svelte, Astro, and vanilla JavaScript. Themes and utility
definitions live in a CSS entrypoint. Conditions and dynamic values use generated
runtime helpers when needed.

## Get started

Follow the [Stillwind package guide](packages/core/README.md) to add a stylesheet,
configure a project, and write styles. Register a [build integration](packages/integration/README.md)
for your application.

| Package                                                         | Use                                                                            |
| --------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| [stillwind](packages/core/README.md)                            | Styles, themes, configuration helpers, and framework APIs                      |
| [@stillwind/integration](packages/integration/README.md)        | Vite, Astro, Nuxt, SvelteKit, Next.js, webpack, Rspack, Rsbuild, and Turbopack |
| [@stillwind/cli](packages/cli/README.md)                        | Sorting, linting, automatic fixes, and CI checks                               |
| [prettier-plugin-stillwind](packages/prettier-plugin/README.md) | Utility sorting during Prettier formatting                                     |
| [VS Code extension](packages/vscode-extension/README.md)        | Diagnostics, completion, CSS previews, colors, and sorting                     |
| [@stillwind/plugin-iconify](plugins/iconify/README.md)          | Iconify icons as utilities                                                     |

## Learn more

- [Style APIs](docs/macro-contract.md): composition, refs, animations, and dynamic values.
- [Project configuration](docs/standalone-analysis.md#shared-project-configuration): project scopes, lint policies, and sorting.
- [Integration guide](docs/integrations.md): framework setup, SSR, and CSS delivery.
- [CSS ordering](docs/css-ordering.md): how composed styles participate in the cascade.

For repository setup, architecture, and verification, see [Development](DEVELOPMENT.md).
