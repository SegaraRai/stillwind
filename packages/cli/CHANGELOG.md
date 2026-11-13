# @stillwind/cli

## 0.10.1

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.10.1
  - @stillwind/language-server@0.10.1
  - @stillwind/native@0.10.1

## 0.10.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.10.0
  - @stillwind/language-server@0.10.0
  - @stillwind/native@0.10.0

## 0.9.0

### Patch Changes

- Each published npm package and the VS Code extension now include `CHANGELOG.md`, so you can read the release notes of the installed version in `node_modules` or on the extension page.

- Updated dependencies:
  - @stillwind/analysis@0.9.0
  - @stillwind/language-server@0.9.0
  - @stillwind/native@0.9.0

## 0.8.0

### Minor Changes

- `stillwind lint` prints a `docs:` line with the page that explains each finding, and the GitHub output includes it in the annotation. The language server links the code of each diagnostic to its page on the website.

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.8.0
  - @stillwind/language-server@0.8.0
  - @stillwind/native@0.8.0

## 0.7.0

### Patch Changes

- The npm package page now shows that each package includes TypeScript declarations. Packages without a root JavaScript entry declare an empty root module, so TypeScript setups that ignore `exports` report a missing export instead of resolving unrelated types.

- Updated dependencies:
  - @stillwind/analysis@0.7.0
  - @stillwind/language-server@0.7.0
  - @stillwind/native@0.7.0

## 0.6.0

### Patch Changes

- The packages now have npm keywords, so the npm search, the Astro integrations library, and other directories that read `keywords` can find them by the frameworks, build tools, and editors that they support. Each package description now states what the package does and which tools it supports.

- Updated dependencies:
  - @stillwind/analysis@0.6.0
  - @stillwind/language-server@0.6.0
  - @stillwind/native@0.6.0

## 0.5.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.5.0
  - @stillwind/language-server@0.5.0
  - @stillwind/native@0.5.0

## 0.4.1

### Patch Changes

- This release publishes the changes listed for 0.4.0. Version 0.4.0 was not published to npm or the VS Code extension marketplaces.

- Updated dependencies:
  - @stillwind/analysis@0.4.1
  - @stillwind/language-server@0.4.1
  - @stillwind/native@0.4.1

## 0.4.0

### Minor Changes

- Findings in `--format json` output now name the documentation page of their code in `docs`. The field uses the page in the installed `stillwind` package, or the website when the package has no such page.

- `stillwind utilities` and `stillwind variants` now show the canonical spelling of an exact name. Pass a CSS declaration as an arbitrary property, such as `[padding:16px]`, to find its utility without changing a file.

- `stillwind inspect` shows the CSS that source code generates. Select files, lines, or positions with paths and globs, and add `--format json` to read the result from scripts and coding agents.

- `stillwind theme`, `stillwind utilities`, and `stillwind variants` show the tokens, utilities, and variants of a project without an editor. Check exact names, search with `*` patterns, and receive the closest spelling for an unknown name.

- Editors that support pull diagnostics, including VS Code, now show Stillwind diagnostics for the files in open tabs. A file that is not open no longer shows diagnostics from a file that imports it. Other editors keep receiving published diagnostics.

### Patch Changes

- On Windows, VS Code no longer opens console windows for the TypeScript checker.

- On Windows, hover previews and other editor features now work in a file that another open file imports.

- On Windows, diagnostics in a file that another open file imports no longer disappear when you open that file, and their quick fixes now apply.

- Declaration files such as `vite-env.d.ts` that match a `*.ts` glob in `files` no longer make `stillwind lint` fail or show an analysis error in the editor.

- Changes to files that analysis never read no longer interrupt it. Other changes to closed files refresh analysis without restarting it, so editor features stay available during builds.

- Updated dependencies:
  - @stillwind/analysis@0.4.0
  - @stillwind/language-server@0.4.0
  - @stillwind/native@0.4.0

## 0.3.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.3.0
  - @stillwind/language-server@0.3.0
  - @stillwind/native@0.3.0

## 0.2.0

### Minor Changes

- The editor now finds the nearest `stillwind.config.*` from each opened file, so a monorepo works without a configuration file at its root.

### Patch Changes

- The Linux native bindings now load on glibc 2.17 and later. They previously failed to load on systems older than glibc 2.35, such as Debian 11 and Red Hat Enterprise Linux 9.

- The editor now shows a configuration error on the configuration file instead of on every open document.

- Updated dependencies:
  - @stillwind/analysis@0.2.0
  - @stillwind/language-server@0.2.0
  - @stillwind/native@0.2.0

## 0.1.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.1.0
  - @stillwind/language-server@0.1.0
  - @stillwind/native@0.1.0
