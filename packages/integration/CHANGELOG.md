# @stillwind/integration

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
