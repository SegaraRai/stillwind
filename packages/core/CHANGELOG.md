# stillwind

## 0.6.0

### Minor Changes

- An `@import` inside a style rule, or inside a `@layer`, `@supports`, `@media`, or other conditional group, now inlines the imported stylesheet in place, as Tailwind CSS v4 does. For example, `.widget { @import "stillwind/preflight.css"; }` applies the reset only to the descendants of `.widget`. Previously, the compiled CSS kept the raw `@import`, and browsers ignored it. Only plain CSS can be imported inside a style rule: when the imported file or its imports contain a definition, `@stillwind-slot`, or a `@layer` statement, such as `stillwind` itself, the import is left out. `invalid-css-import` reports that import and an unresolved nested import at their source locations.

  A definition such as `@utility` or `@theme`, a Tailwind statement such as `@plugin`, or a `@layer` statement written inside a style rule is now ignored and reported by `invalid-css-directive`. Previously, a nested `@utility` was registered while a nested `@theme` was not, the raw at-rules stayed in the compiled CSS, and a nested `@layer` statement failed the build with a parse error.

### Patch Changes

- The migration guides now describe how to check a migration: compare the computed styles of the migrated components in the old and new builds with a browser console function. The `migrate-to-stillwind` skill now asks agents to run this comparison for each theme and breakpoint and to report each difference.

- A `@variant` block in the body of a `@utility` now applies its variants, as it does in a style rule. Directly in the body, the variants apply to the element that uses the utility. Inside a nested rule, they apply to that rule's selector. Previously, the compiled CSS kept the raw `@variant` rule, and browsers ignored those styles. `prefer-variant-directive` now also reports conditions in `@utility` bodies that a variant can replace.

- The packages now have npm keywords, so the npm search, the Astro integrations library, and other directories that read `keywords` can find them by the frameworks, build tools, and editors that they support. Each package description now states what the package does and which tools it supports.

- Updated dependencies:
  - @stillwind/config@0.6.0
  - @stillwind/native@0.6.0
  - @stillwind/projection@0.6.0

## 0.5.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.5.0
  - @stillwind/native@0.5.0
  - @stillwind/projection@0.5.0

## 0.4.1

### Patch Changes

- This release publishes the changes listed for 0.4.0. Version 0.4.0 was not published to npm or the VS Code extension marketplaces.

- Updated dependencies:
  - @stillwind/config@0.4.1
  - @stillwind/native@0.4.1
  - @stillwind/projection@0.4.1

## 0.4.0

### Patch Changes

- Arbitrary properties that use `--if()`, such as `[color:--if(--on,red,blue)]`, no longer report `incompatible-property-value` when the property accepts both branches. The rule still reports a branch that the property rejects.

- `if()` variants now work in `style.css()`. The returned string no longer contains an internal at-rule that kept browsers from applying the conditional rule.

- Variants that test a shadow host now match. `host:hover:`, `hover:` with `selector: ":host"`, and `@apply hover:flex` inside `:host` generate `:host()` forms that browsers accept. The new `unmatched-host-selector` rule reports variants such as `host:in-[.page]:`, which read elements outside the shadow tree and cannot match.

- Arbitrary properties whose name is not a CSS property name, such as `[&:focus]`, `[:focus]`, and `[1:2]`, now report `invalid-utility` and emit no CSS. Add a utility after a variant, as in `[&:focus]:flex`, to style with a selector.

- The license comment at the start of generated CSS now shows the installed Stillwind version.

- The bundled `docs/utilities.md` page is now `docs/utilities-and-variants.md`, and the website page moved to `/docs/utilities-and-variants`. The lookup on the website page now also finds variants by name or by part of their CSS, such as `:hover` or `media hover`.

- Updated dependencies:
  - @stillwind/config@0.4.0
  - @stillwind/native@0.4.0
  - @stillwind/projection@0.4.0

## 0.3.0

### Minor Changes

- The `recommended` preset now reports `unknown-utility` and `unknown-variant` as errors, so `stillwind lint` fails on a misspelled utility or variant. Set a rule to `warn` in `lint.rules` to keep the previous severity.

  `prefer-explicit-nesting-selector` is now `implicit-nesting-selector`, and its category changes from `style` to `hazard`. Update the rule name in `lint.rules` and in `swlint-*` comments.

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.3.0
  - @stillwind/native@0.3.0
  - @stillwind/projection@0.3.0

## 0.2.0

### Minor Changes

- The editor now finds the nearest `stillwind.config.*` from each opened file, so a monorepo works without a configuration file at its root.

- Reference configuration files, directories, and globs from `projects`, so each package in a monorepo can keep its own `stillwind.config.ts`.

- Import a file from `stillwind/logical/` after `stillwind` to make built-in axis utilities follow the writing direction, as `px-*` and `py-*` do. `overflow.css`, `overscroll.css`, `divide.css`, `snap.css`, and `resize.css` cover `overflow-x-*`, `overflow-y-*`, `overscroll-x-*`, `overscroll-y-*`, `divide-y-*`, `snap-x`, `snap-y`, `resize-x`, and `resize-y`. `prefer-logical-properties` no longer reports the `overflow`, `overscroll`, and `divide` utilities that these files change.

### Patch Changes

- `prefer-canonical-utilities` no longer replaces one theme token with another that only has the same value, such as `w-96` with `w-sm`. Fixed values now prefer spacing multiples, such as `w-112` for `w-[448px]`, and stay as written when several unrelated named tokens match.

- `prefer-logical-properties` no longer reports a shorthand that gives both axes the same value, such as `overflow-hidden` or `truncate`, and it no longer reports `sr-only` or `not-sr-only`, whose declarations you cannot change. `no-restricted-length-units` also skips `sr-only` by default.

- The Linux native bindings now load on glibc 2.17 and later. They previously failed to load on systems older than glibc 2.35, such as Debian 11 and Red Hat Enterprise Linux 9.

- `prefer-logical-properties` now reports physical declarations that it previously missed: shorthands for some sides with any value, such as `[border-top:var(--border)]` and `[border-left-radius:1px]`, and shorthands with different values for each side, such as `[padding:var(--a)_1rem]` and `[size:10px_20px]`.

- The editor now shows a configuration error on the configuration file instead of on every open document.

- `prefer-canonical-utilities` no longer rewrites an arbitrary value that starts and ends with separate function calls, such as `grid-cols-[var(--sidebar)_minmax(0,1fr)]`, into broken classes. `stillwind lint --fix` now keeps these values as written.

- Updated dependencies:
  - @stillwind/config@0.2.0
  - @stillwind/native@0.2.0
  - @stillwind/projection@0.2.0

## 0.1.0

### Patch Changes

- Release the first version of Stillwind.

- Updated dependencies:
  - @stillwind/config@0.1.0
  - @stillwind/native@0.1.0
  - @stillwind/projection@0.1.0
