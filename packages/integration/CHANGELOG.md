# @stillwind/integration

## 0.7.0

### Patch Changes

- Astro sites that use the ClientRouter now keep Stillwind styles from client scripts after a client-side navigation. Previously, the stylesheet that a client script loaded was removed from the page head when the router swapped pages, so elements that the script created stayed unstyled until a full reload.

- Rules that a Tailwind CSS JavaScript plugin nests inside `@scope` within a selector, such as `@scope (&)`, now stay inside the `@scope` block, as the same rules in a CSS `@utility` do. Previously, each nested selector moved into the `@scope` prelude, so a selector that used `:scope` matched the document root instead of the scoping root and the rule never applied. For example, daisyUI's `join` lost the radius of its outer corners.

  A plugin that passes `addUtilities()` or `addComponents()` an `@scope` rule outside any selector, such as `{ "@scope (.theme)": { ".item": { … } } }`, now fails with an error. Previously, Stillwind generated CSS that styled other elements than Tailwind CSS does, without a diagnostic.

- A SvelteKit server that uses `@stillwind/integration/sveltekit/handle` now starts when the package is installed from npm. Previously, the SSR build left the handle as an external import, so a production server, for example one built with `@sveltejs/adapter-node`, failed to start with `Cannot find package '$app'`. You can remove an `ssr.noExternal` entry that you added for `@stillwind/integration` as a workaround.

- Astro components whose frontmatter ends with a `//` line comment now pass `stillwind lint` and the build. Previously, such components failed with SW31005 ("Astro frontmatter coordinates do not match source"), so a trailing comment such as `// TODO` or a tool directive could fail the whole site build.

- The npm package page now shows that each package includes TypeScript declarations. Packages without a root JavaScript entry declare an empty root module, so TypeScript setups that ignore `exports` report a missing export instead of resolving unrelated types.

- Updated dependencies:
  - @stillwind/config@0.7.0
  - @stillwind/host@0.7.0
  - @stillwind/native@0.7.0
  - stillwind@0.7.0

## 0.6.0

### Patch Changes

- Vite-based integrations, including Astro, now warn when application code or a framework option such as Starlight's `customCss` imports the CSS entrypoint selected in `stillwind.config.ts`. The integration already delivers the compiled CSS, and importing the entrypoint also sends its uncompiled `@theme` and `@utility` rules to the page. Remove the import to fix the warning.

- In development, pages that link `/__stillwind/style.css?environment=ssr` now reload that stylesheet when the CSS changes. Theme edits now reach pages that never load the Stillwind client runtime, such as resumable Qwik pages, without importing the CSS entrypoint or reloading the page.

- The packages now have npm keywords, so the npm search, the Astro integrations library, and other directories that read `keywords` can find them by the frameworks, build tools, and editors that they support. Each package description now states what the package does and which tools it supports.

- Updated dependencies:
  - @stillwind/config@0.6.0
  - @stillwind/host@0.6.0
  - @stillwind/native@0.6.0
  - stillwind@0.6.0

## 0.5.0

### Patch Changes

- During Vite development, server rendering now shows a component edit even when a framework transforms the edited file for the server before the hot update runs. Previously, TanStack Start could keep rendering the component from before the edit until the server restarted.

- Updated dependencies:
  - @stillwind/config@0.5.0
  - @stillwind/host@0.5.0
  - @stillwind/native@0.5.0
  - stillwind@0.5.0

## 0.4.1

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.4.1
  - @stillwind/host@0.4.1
  - @stillwind/native@0.4.1
  - stillwind@0.4.1

## 0.4.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.4.0
  - @stillwind/host@0.4.0
  - @stillwind/native@0.4.0
  - stillwind@0.4.0

## 0.3.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.3.0
  - @stillwind/host@0.3.0
  - @stillwind/native@0.3.0
  - stillwind@0.3.0

## 0.2.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.2.0
  - @stillwind/host@0.2.0
  - @stillwind/native@0.2.0
  - stillwind@0.2.0

## 0.1.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.1.0
  - @stillwind/host@0.1.0
  - @stillwind/native@0.1.0
  - stillwind@0.1.0
