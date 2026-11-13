# @stillwind/language-server

## 0.10.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.10.0
  - @stillwind/native@0.10.0

## 0.9.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.9.0
  - @stillwind/native@0.9.0

## 0.8.0

### Minor Changes

- `stillwind lint` prints a `docs:` line with the page that explains each finding, and the GitHub output includes it in the annotation. The language server links the code of each diagnostic to its page on the website.

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.8.0
  - @stillwind/native@0.8.0

## 0.7.0

### Patch Changes

- The npm package page now shows that each package includes TypeScript declarations. Packages without a root JavaScript entry declare an empty root module, so TypeScript setups that ignore `exports` report a missing export instead of resolving unrelated types.

- Updated dependencies:
  - @stillwind/analysis@0.7.0
  - @stillwind/native@0.7.0

## 0.6.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.6.0
  - @stillwind/native@0.6.0

## 0.5.0

### Minor Changes

- The Stillwind output now shows the extension and bundled server versions, why the bundled server runs, which configurations opened and how long opening took, and the workspace folders without a configuration. Debug level shows the event that reloads configurations, and Error entries include stack traces for unexpected failures. A repeated editor request failure appears once until configurations reload. The Web extension now writes to the same output, including the number of captured workspace files.

  A configuration error now notifies once until the error ends, even when its message changes with each edit, and the notification is an error instead of a warning. When the project's `@stillwind/cli` fails to start, VS Code shows a warning that the bundled server runs instead. The new `stillwind.trace.server` setting selects the detail of the LSP trace, which the Stillwind output shows at Trace level.

- Color decorators now show two swatches, light then dark, for a utility or theme reference whose color uses `light-dark()` with different colors. The color picker does not edit these colors, because one color cannot replace both arguments.

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.5.0
  - @stillwind/native@0.5.0

## 0.4.1

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.4.1
  - @stillwind/native@0.4.1

## 0.4.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.4.0
  - @stillwind/native@0.4.0

## 0.3.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.3.0
  - @stillwind/native@0.3.0

## 0.2.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.2.0
  - @stillwind/native@0.2.0

## 0.1.0

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.1.0
  - @stillwind/native@0.1.0
