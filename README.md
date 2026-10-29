# Stillwind

**Familiar utilities. Enforced rules.**

Stillwind is a compiler for component styles. Write Tailwind CSS v4 utilities,
enforce your design system, and ship static CSS. Its checks keep styling code
in order, even when AI agents write much of it.

[Website](https://stillwind.dev/) · [Documentation](https://stillwind.dev/docs/) ·
[Playground](https://play.stillwind.dev/) · [Coding agents](https://stillwind.dev/docs/agents/)

> [!CAUTION]
> Stillwind is in early development. Test your application's build and pages before you release changes.

```css
/* src/stillwind.css */
@import "stillwind";

@theme {
  --color-accent: oklch(0.6 0.2 250);
}
```

```tsx
import { style } from "stillwind/react";

<button {...style("rounded-md bg-accent px-4 py-2 text-white", loading && "opacity-60")}>
  Save
</button>;
```

`style()` returns props, such as `className`, for the element. The compiler
generates static CSS for the utilities, and the CLI reports a utility that the
theme does not define, such as a misspelled `bg-acent`.
[Try it in the Playground](https://play.stillwind.dev/#preset=release-checklist).

## Why Stillwind

- [Design systems](https://stillwind.dev/docs/guides/design-systems/): require colors,
  spacing, and breakpoints to come from your theme.
- [Element relationships](https://stillwind.dev/docs/guides/element-relationships/):
  check that styles depending on a parent or sibling element actually refer to that element.
- [Animations](https://stillwind.dev/docs/guides/animations/) and
  [view transitions](https://stillwind.dev/docs/guides/view-transitions/): define them
  inside the component that uses them, without adding global names.

[Why Stillwind](https://stillwind.dev/docs/why/) explains these checks in detail.

## Get started

Install the packages:

```sh
vp i stillwind
vp i -D @stillwind/integration @stillwind/cli
```

Then follow [Getting started](https://stillwind.dev/docs/) to add a stylesheet,
configure the project, and register the build integration.

To let a coding agent do the setup, give it this prompt:

```text
Set up Stillwind in this project. If the stillwind package is not installed,
install it with the project's package manager. Then read
node_modules/stillwind/docs/agents.md and follow its instructions for agents.
```

The agent sets up a new project or migrates an existing one. Before it migrates
existing styles, it asks how far to go. See [Coding agents](https://stillwind.dev/docs/agents/).

## Frameworks and build tools

Stillwind supports Alpine, Astro, Lit, Preact, Qwik, React, Solid, Svelte, vanilla
JavaScript, and Vue. Its build integration supports Vite, Astro, Fresh, Nuxt,
SvelteKit, Next.js, webpack, Rspack, Rsbuild, and Turbopack. See
[Framework integrations](https://stillwind.dev/docs/integrations/).

Stillwind accepts Tailwind CSS v4 utilities, theme variables, and CSS directives.
To move an existing project, see
[Migrating from Tailwind CSS](https://stillwind.dev/docs/guides/migrating-from-tailwind/)
or [Migrating from CSS](https://stillwind.dev/docs/guides/migrating-from-css/).

## Packages

| Package                                                         | Use                                                        |
| --------------------------------------------------------------- | ---------------------------------------------------------- |
| [stillwind](packages/core/README.md)                            | Styles, themes, configuration helpers, and framework APIs  |
| [@stillwind/integration](packages/integration/README.md)        | Build integrations                                         |
| [@stillwind/cli](packages/cli/README.md)                        | Sorting, linting, automatic fixes, and CI checks           |
| [prettier-plugin-stillwind](packages/prettier-plugin/README.md) | Utility sorting during Prettier formatting                 |
| [VS Code extension](packages/vscode-extension/README.md)        | Diagnostics, completion, CSS previews, colors, and sorting |
| [@stillwind/plugin-iconify](plugins/iconify/README.md)          | Iconify icons as utilities                                 |

## Contributing

[Development](DEVELOPMENT.md) describes the repository setup, architecture, and
verification. [PHILOSOPHY.md](PHILOSOPHY.md) records the design principles.

## License

Stillwind is released under the [MIT License](LICENSE). See
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) for the licenses of included
third-party code.
