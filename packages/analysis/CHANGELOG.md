# @stillwind/analysis

## 0.13.0

### Minor Changes

- `stillwind lint`, `fmt`, and `check` now analyze the projects of a configuration in parallel worker processes, so a configuration that lists several projects, such as a monorepo root, finishes sooner. Projects that share a target file are still analyzed together in one process. `lint`, `format`, and `check` in `@stillwind/analysis` accept a `concurrency` option that sets the maximum number of worker processes. By default, the maximum is half of the available parallelism.

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.13.0
  - @stillwind/host@0.13.0
  - @stillwind/native@0.13.0

## 0.12.0

### Minor Changes

- Added `stillwind check` and `stillwind check --fix` to run lint checks and utility sorting in a single analysis pass. `stillwind check` reports lint findings and files that need sorting without saving, and exits with `1` when errors or unsorted files remain. `stillwind check --fix` applies converged automatic fixes and utility sorting, saves changed files, and reports findings for the final sources. `@stillwind/analysis/node` exports the corresponding `check()` function, and `stillwind`'s agent instructions now recommend `stillwind check --fix`.

- Added `stillwind info` (and `info()` in `@stillwind/analysis/node`) to report each project's resolved root, stylesheet path, target and exclude globs, matched file count, registered plugins, sorting and `typeFacts` settings, and active lint rules in text or JSON format without compiling source files.

- Diagnostic actions in CLI output now distinguish automatic fixes from manual suggestions (`actions[].autofix`, `summary.autofixable`, and `(autofix)` / `(suggestion)` labels in text output), and the `--fix` hint is shown only when automatic fixes are available. `stillwind inspect` now explains why no previews were returned (`emptyReason`: `no-styles`, `line-out-of-range`, `column-out-of-range`, or `no-match-at-position`). Uncompiled runtime placeholder errors for every `style.*` macro now include guidance on build integration ordering and `docs/integrations.md`.

- Added project-scoped `hooks` (`defineHook` and `AnalysisHook` from `stillwind/config` and `@stillwind/config`) for in-memory formatting in `stillwind fmt` and `stillwind check`. Hooks support `setup`, `teardown`, and per-file `format` callbacks so formatters such as Oxfmt run after Stillwind utility sorting in one pass, including `--check`. `stillwind info` now also lists configured hook names.

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.12.0
  - @stillwind/host@0.12.0
  - @stillwind/native@0.12.0

## 0.11.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.11.0
  - @stillwind/host@0.11.0
  - @stillwind/native@0.11.0

## 0.10.1

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.10.1
  - @stillwind/host@0.10.1
  - @stillwind/native@0.10.1

## 0.10.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.10.0
  - @stillwind/host@0.10.0
  - @stillwind/native@0.10.0

## 0.9.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.9.0
  - @stillwind/host@0.9.0
  - @stillwind/native@0.9.0

## 0.8.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.8.0
  - @stillwind/host@0.8.0
  - @stillwind/native@0.8.0

## 0.7.0

### Patch Changes

- The npm package page now shows that each package includes TypeScript declarations. Packages without a root JavaScript entry declare an empty root module, so TypeScript setups that ignore `exports` report a missing export instead of resolving unrelated types.

- Updated dependencies:
  - @stillwind/config@0.7.0
  - @stillwind/host@0.7.0
  - @stillwind/native@0.7.0

## 0.6.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.6.0
  - @stillwind/host@0.6.0
  - @stillwind/native@0.6.0

## 0.5.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.5.0
  - @stillwind/host@0.5.0
  - @stillwind/native@0.5.0

## 0.4.1

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.4.1
  - @stillwind/host@0.4.1
  - @stillwind/native@0.4.1

## 0.4.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.4.0
  - @stillwind/host@0.4.0
  - @stillwind/native@0.4.0

## 0.3.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.3.0
  - @stillwind/host@0.3.0
  - @stillwind/native@0.3.0

## 0.2.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.2.0
  - @stillwind/host@0.2.0
  - @stillwind/native@0.2.0

## 0.1.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.1.0
  - @stillwind/host@0.1.0
  - @stillwind/native@0.1.0
