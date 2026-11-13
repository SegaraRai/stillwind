# @stillwind/vscode-extension

## 0.10.1

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.10.1
  - @stillwind/config@0.10.1
  - @stillwind/host@0.10.1
  - @stillwind/language-server@0.10.1
  - @stillwind/native@0.10.1

## 0.10.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.10.0
  - @stillwind/config@0.10.0
  - @stillwind/host@0.10.0
  - @stillwind/language-server@0.10.0
  - @stillwind/native@0.10.0

## 0.9.0

### Patch Changes

- Each published npm package and the VS Code extension now include `CHANGELOG.md`, so you can read the release notes of the installed version in `node_modules` or on the extension page.

- Updated dependencies:
  - @stillwind/analysis@0.9.0
  - @stillwind/config@0.9.0
  - @stillwind/host@0.9.0
  - @stillwind/language-server@0.9.0
  - @stillwind/native@0.9.0

## 0.8.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.8.0
  - @stillwind/config@0.8.0
  - @stillwind/host@0.8.0
  - @stillwind/language-server@0.8.0
  - @stillwind/native@0.8.0

## 0.7.0

### Patch Changes

- The npm package page now shows that each package includes TypeScript declarations. Packages without a root JavaScript entry declare an empty root module, so TypeScript setups that ignore `exports` report a missing export instead of resolving unrelated types.

- Updated dependencies:
  - @stillwind/analysis@0.7.0
  - @stillwind/config@0.7.0
  - @stillwind/host@0.7.0
  - @stillwind/language-server@0.7.0
  - @stillwind/native@0.7.0

## 0.6.0

### Patch Changes

- The VS Code extension now starts when you open a Vue, Svelte, or Astro file, even if the workspace root has no `stillwind.config.*`. Previously, a workspace with its configuration only in a subfolder got no diagnostics or completion in these files until a JavaScript, TypeScript, or CSS file was opened. The Marketplace and Open VSX listings now show keywords, a banner, and a description that names Tailwind CSS v4.

- Updated dependencies:
  - @stillwind/analysis@0.6.0
  - @stillwind/config@0.6.0
  - @stillwind/host@0.6.0
  - @stillwind/language-server@0.6.0
  - @stillwind/native@0.6.0

## 0.5.0

### Minor Changes

- The Stillwind output now shows the extension and bundled server versions, why the bundled server runs, which configurations opened and how long opening took, and the workspace folders without a configuration. Debug level shows the event that reloads configurations, and Error entries include stack traces for unexpected failures. A repeated editor request failure appears once until configurations reload. The Web extension now writes to the same output, including the number of captured workspace files.

  A configuration error now notifies once until the error ends, even when its message changes with each edit, and the notification is an error instead of a warning. When the project's `@stillwind/cli` fails to start, VS Code shows a warning that the bundled server runs instead. The new `stillwind.trace.server` setting selects the detail of the LSP trace, which the Stillwind output shows at Trace level.

- Color decorators now show two swatches, light then dark, for a utility or theme reference whose color uses `light-dark()` with different colors. The color picker does not edit these colors, because one color cannot replace both arguments.

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.5.0
  - @stillwind/config@0.5.0
  - @stillwind/host@0.5.0
  - @stillwind/language-server@0.5.0
  - @stillwind/native@0.5.0

## 0.4.1

### Patch Changes

- This release publishes the changes listed for 0.4.0. Version 0.4.0 was not published to npm or the VS Code extension marketplaces.

- Updated dependencies:
  - @stillwind/analysis@0.4.1
  - @stillwind/config@0.4.1
  - @stillwind/host@0.4.1
  - @stillwind/language-server@0.4.1
  - @stillwind/native@0.4.1

## 0.4.0

### Minor Changes

- Editors that support pull diagnostics, including VS Code, now show Stillwind diagnostics for the files in open tabs. A file that is not open no longer shows diagnostics from a file that imports it. Other editors keep receiving published diagnostics.

### Patch Changes

- The VS Code extension now analyzes Astro, Svelte, and Vue files in projects that do not install `@stillwind/cli`.

- On Windows, VS Code no longer opens console windows for the TypeScript checker.

- On Windows, hover previews and other editor features now work in a file that another open file imports.

- On Windows, diagnostics in a file that another open file imports no longer disappear when you open that file, and their quick fixes now apply.

- Declaration files such as `vite-env.d.ts` that match a `*.ts` glob in `files` no longer make `stillwind lint` fail or show an analysis error in the editor.

- The Stillwind output in VS Code no longer repeats Crashpad errors.

- Changes to files that analysis never read no longer interrupt it. Other changes to closed files refresh analysis without restarting it, so editor features stay available during builds.

- The VS Code extension now loads a configuration only when a tab shows a file that uses it. Files that other extensions open in the background no longer start analysis processes or report configuration errors.

- Updated dependencies:
  - @stillwind/analysis@0.4.0
  - @stillwind/config@0.4.0
  - @stillwind/host@0.4.0
  - @stillwind/language-server@0.4.0
  - @stillwind/native@0.4.0

## 0.3.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.3.0
  - @stillwind/config@0.3.0
  - @stillwind/host@0.3.0
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
  - @stillwind/config@0.2.0
  - @stillwind/host@0.2.0
  - @stillwind/language-server@0.2.0
  - @stillwind/native@0.2.0

## 0.1.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.1.0
  - @stillwind/config@0.1.0
  - @stillwind/host@0.1.0
  - @stillwind/language-server@0.1.0
  - @stillwind/native@0.1.0
