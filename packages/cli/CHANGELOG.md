# @stillwind/cli

## 0.13.0

### Minor Changes

- CSS `@variant` rules now accept `or(...)` variant alternatives, such as `@variant md:or(hover,focus) { ... }`.

- `stillwind lint`, `fmt`, and `check` now analyze the projects of a configuration in parallel worker processes, so a configuration that lists several projects, such as a monorepo root, finishes sooner. Projects that share a target file are still analyzed together in one process. `lint`, `format`, and `check` in `@stillwind/analysis` accept a `concurrency` option that sets the maximum number of worker processes. By default, the maximum is half of the available parallelism.

### Patch Changes

- `prefer-canonical-utilities` no longer suggests a replacement that matches only for the current theme values. For example, it no longer suggests `rounded-md` for `rounded-[min(var(--radius-md),10px)]`, because the two differ when `--radius-md` is larger than `10px`.

  It also no longer uses the built-in value of `--radius` for a theme value that refers to `var(--radius)`, because the generated CSS does not declare that value. For example, with `--radius-lg: var(--radius)` in `@theme inline` and `--radius` declared in `:root`, it no longer suggests `rounded-lg` for `rounded-[4px]`.

- Hover previews now annotate conditional utilities and recipe variants/slots with rule-leading comments, show scoped previews when hovering recipe slots, variant axes, variant options, compound entries, and style sub-expressions, and filter `recipe({ ... })` call previews by static arguments and `defaultVariants`.

- Hover previews now annotate `color-mix()` and `light-dark()` color equivalents, evaluate `calc()` with `rem` dimensions when `themeValues` is disabled and inside `@media` / `@container` preludes, display registered `@property` definitions on `@stillwind-require-properties` arguments, preview `style.when()`, `style.ref()`, and `style.recipe()` calls and their variable references, and display the resolved JavaScript string literal for `style.media()` calls and references.

- Hover previews in the language server now omit redundant empty `& {}` rules inside selector variants, trim trailing blank lines from CSS code blocks, annotate active theme declarations hovered in stylesheets, omit redundant hexadecimal comments on shorthand hex theme tokens, omit marker-class notes under enclosing variants, and avoid treating unrelated `.at()` and `.in()` calls as ref variants.

- Analysis no longer prepares the TypeScript checker again each time CSS ordering reads a module that the analyzed files do not import for type information. `stillwind lint`, `fmt`, and `check` and the editor finish sooner in projects whose components are imported through other modules.

- Show marker documentation instead of a variant CSS block when hovering `style.ref()` declarations and ref marker references.

- Fix hover and completion previews for `if(...)`, `or(...)`, and `@scope` variants, and remove trailing spaces in `@starting-style` completion details.

- Variant hover and completion previews now mark where utility declarations or nested rules are inserted with `/* … */`.

- Updated dependencies:
  - @stillwind/analysis@0.13.0
  - @stillwind/language-server@0.13.0
  - @stillwind/native@0.13.0

## 0.12.0

### Minor Changes

- Added `stillwind check` and `stillwind check --fix` to run lint checks and utility sorting in a single analysis pass. `stillwind check` reports lint findings and files that need sorting without saving, and exits with `1` when errors or unsorted files remain. `stillwind check --fix` applies converged automatic fixes and utility sorting, saves changed files, and reports findings for the final sources. `@stillwind/analysis/node` exports the corresponding `check()` function, and `stillwind`'s agent instructions now recommend `stillwind check --fix`.

- Added `stillwind info` (and `info()` in `@stillwind/analysis/node`) to report each project's resolved root, stylesheet path, target and exclude globs, matched file count, registered plugins, sorting and `typeFacts` settings, and active lint rules in text or JSON format without compiling source files.

- Add `--limit <n>` to `stillwind check` and `stillwind lint` to cap displayed diagnostics while preserving full counts and exit codes, add `summary.byCode` and `summary.omittedDiagnostics` to `--format json`, and expose working-directory-relative `file` paths with 1-based `line` and `column` fields directly on JSON diagnostics and inspect previews.

- `stillwind utilities` and `stillwind variants` now accept `--names-only` to list matching names without formatting CSS previews. Pattern searches in `stillwind utilities` now also include placeholder patterns for functional utilities' non-enumerable domains, such as `p-<number>` and `p-[<length-percentage>]` (`kind: "pattern"` in JSON). `stillwind theme` now includes resolved values for tokens that reference other theme variables (`resolvedValue` in JSON, and inline comments in text output).

- Diagnostic actions in CLI output now distinguish automatic fixes from manual suggestions (`actions[].autofix`, `summary.autofixable`, and `(autofix)` / `(suggestion)` labels in text output), and the `--fix` hint is shown only when automatic fixes are available. `stillwind inspect` now explains why no previews were returned (`emptyReason`: `no-styles`, `line-out-of-range`, `column-out-of-range`, or `no-match-at-position`). Uncompiled runtime placeholder errors for every `style.*` macro now include guidance on build integration ordering and `docs/integrations.md`.

- Added project-scoped `hooks` (`defineHook` and `AnalysisHook` from `stillwind/config` and `@stillwind/config`) for in-memory formatting in `stillwind fmt` and `stillwind check`. Hooks support `setup`, `teardown`, and per-file `format` callbacks so formatters such as Oxfmt run after Stillwind utility sorting in one pass, including `--check`. `stillwind info` now also lists configured hook names.

### Patch Changes

- The `stillwind` package now bundles `docs/llms.txt`, a single-file index of all bundled Markdown documentation pages with relative paths and descriptions, and updates `AGENTS_TEMPLATE.md` and `skills/stillwind/SKILL.md` so coding agents can look up design-system tokens, utilities, and generated CSS without broken cross-skill links. The public `style()` overload now appears before the internal checker overload so TypeScript diagnostics report the public signature first. `stillwind inspect` now prints `docs:` lines for findings in text output, matching `stillwind lint`.

- Make editor integrations clearer. The VS Code extension shows a Stillwind status bar item that tells whether the project's `@stillwind/cli` or the bundled language server runs, and opens the output channel on click. A change to `stillwind.configFile` restarts the language server. **Stillwind: Sort Utilities** reports in the status bar when the utilities are already sorted. Stillwind at-rules in CSS files show descriptions and syntax on hover and in completion, and the Command Palette no longer lists internal commands. Diagnostics use `stillwind` as their source, and add the project path only when several projects report on the same file. Color utilities complete with color swatches, completion lists report when the catalog has more entries than they show, resolved completions keep their detail, Quick Fixes link to their diagnostics and mark the primary fix as preferred, and hovers name the project only when several projects give different results. The Prettier `stillwindConfig` option describes every `stillwind.config.*` format.

- Make CLI and build output easier to read. `stillwind --help` now highlights headings and options, lists `--format files` only for `stillwind fmt`, and suggests valid commands and `--help` for an unknown command or option. Text output colors file paths, codes, `note:`, `help:`, and `docs:` labels, shows `[project N]` only when results come from several projects, and gives `stillwind inspect` diagnostics the same colors and docs links. Compiler and lint messages no longer show internal byte offsets or debug values, name the conflicting framework and source format for `SW02001`, show the rejected prefix for `SW01001`, and use consistent punctuation and backticks. Unterminated blocks, nesting overflows, and syntax errors in CSS now point at the failing character instead of the whole stylesheet. When the failing CSS has no document URI, such as CSS that a plugin generates, the error message names that source and the byte offset. Vite reports compile errors in the error overlay with a clickable location and a code frame, and build warnings use the `stillwind: <rule>: <message> (<path>:<line>:<column>)` form.

- Updated dependencies:
  - @stillwind/analysis@0.12.0
  - @stillwind/language-server@0.12.0
  - @stillwind/native@0.12.0

## 0.11.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.11.0
  - @stillwind/language-server@0.11.0
  - @stillwind/native@0.11.0

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
