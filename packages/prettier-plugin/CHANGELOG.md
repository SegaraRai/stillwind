# prettier-plugin-stillwind

## 0.13.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.13.0

## 0.12.0

### Patch Changes

- Make editor integrations clearer. The VS Code extension shows a Stillwind status bar item that tells whether the project's `@stillwind/cli` or the bundled language server runs, and opens the output channel on click. A change to `stillwind.configFile` restarts the language server. **Stillwind: Sort Utilities** reports in the status bar when the utilities are already sorted. Stillwind at-rules in CSS files show descriptions and syntax on hover and in completion, and the Command Palette no longer lists internal commands. Diagnostics use `stillwind` as their source, and add the project path only when several projects report on the same file. Color utilities complete with color swatches, completion lists report when the catalog has more entries than they show, resolved completions keep their detail, Quick Fixes link to their diagnostics and mark the primary fix as preferred, and hovers name the project only when several projects give different results. The Prettier `stillwindConfig` option describes every `stillwind.config.*` format.

- Updated dependencies:
  - @stillwind/analysis@0.12.0

## 0.11.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.11.0

## 0.10.1

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.10.1

## 0.10.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.10.0

## 0.9.0

### Patch Changes

- Each published npm package and the VS Code extension now include `CHANGELOG.md`, so you can read the release notes of the installed version in `node_modules` or on the extension page.

- Updated dependencies:
  - @stillwind/analysis@0.9.0

## 0.8.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.8.0

## 0.7.0

### Patch Changes

- The npm package page now shows that each package includes TypeScript declarations. Packages without a root JavaScript entry declare an empty root module, so TypeScript setups that ignore `exports` report a missing export instead of resolving unrelated types.

- Updated dependencies:
  - @stillwind/analysis@0.7.0

## 0.6.0

### Patch Changes

- The packages now have npm keywords, so the npm search, the Astro integrations library, and other directories that read `keywords` can find them by the frameworks, build tools, and editors that they support. Each package description now states what the package does and which tools it supports.

- Updated dependencies:
  - @stillwind/analysis@0.6.0

## 0.5.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.5.0

## 0.4.1

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.4.1

## 0.4.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.4.0

## 0.3.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.3.0

## 0.2.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.2.0

## 0.1.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.1.0
