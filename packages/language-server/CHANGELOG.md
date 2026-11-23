# @stillwind/language-server

## 0.12.0

### Patch Changes

- Make editor integrations clearer. The VS Code extension shows a Stillwind status bar item that tells whether the project's `@stillwind/cli` or the bundled language server runs, and opens the output channel on click. A change to `stillwind.configFile` restarts the language server. **Stillwind: Sort Utilities** reports in the status bar when the utilities are already sorted. Stillwind at-rules in CSS files show descriptions and syntax on hover and in completion, and the Command Palette no longer lists internal commands. Diagnostics use `stillwind` as their source, and add the project path only when several projects report on the same file. Color utilities complete with color swatches, completion lists report when the catalog has more entries than they show, resolved completions keep their detail, Quick Fixes link to their diagnostics and mark the primary fix as preferred, and hovers name the project only when several projects give different results. The Prettier `stillwindConfig` option describes every `stillwind.config.*` format.

- Updated dependencies:
  - @stillwind/analysis@0.12.0
  - @stillwind/native@0.12.0

## 0.11.0

### Minor Changes

- Editors now show hover previews for ref variants, such as `trigger("hover:")`, ``trigger`hover:` ``, `trigger.at("width >= 40rem")`, and `trigger.in(card)`, when they are used in `style()`. The preview shows the generated selector or container query of the variant, and it appears wherever the cursor is in the variant expression. Completion suggests variants in the text of a ref call or tag. In a utility template such as `` `${trigger("hover:")}:block` ``, the utility after the substitution now has a hover preview that includes the substituted variant.

- Editors now show hover previews and completions for variant keys, such as `"hover:"` in `style({ "hover:": "block" })`, including nested keys and the keys in `style.css()` and `style.cva()` inputs. The arguments of `style.when()` get the same hover previews and completions. A hover preview shows the selectors and conditions that the variants add, including the variants of enclosing keys. Completion suggests only variants, also while the key is still incomplete.

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.11.0
  - @stillwind/native@0.11.0

## 0.10.1

### Patch Changes

- Updated dependencies:
  - @stillwind/analysis@0.10.1
  - @stillwind/native@0.10.1

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
