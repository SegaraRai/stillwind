# stillwind

## 0.9.0

### Minor Changes

- Astro components are now converted to TSX for analysis with `@astrojs/astro2tsx`, the converter that the Astro language server uses, instead of `@astrojs/compiler`. For Astro support, install `@astrojs/astro2tsx` `^0.1.4`. Stillwind no longer uses `@astrojs/compiler`.

  Analysis now finds frontmatter in the same places as the Astro 7 build. A `---` fence that follows template text, such as an HTML comment, is template text and does not start frontmatter.

- Type facts now come from [tsrs](https://github.com/oxc-project/tsrs), a Rust port of the TypeScript 7.1 beta checker, instead of TypeScript 7.0. Edits that need type information finish faster. Stillwind no longer installs `typescript` 7.

### Patch Changes

- Update Tailwind CSS compatibility to 4.3.5, preserve empty variable fallbacks, and improve group and peer selectors and Safari property fallbacks.

- Documentation examples now declare component-specific refs, keyframes, and view transitions inside the component that uses them.

- Each published npm package and the VS Code extension now include `CHANGELOG.md`, so you can read the release notes of the installed version in `node_modules` or on the extension page.

- Updated dependencies:
  - @stillwind/config@0.9.0
  - @stillwind/native@0.9.0
  - @stillwind/projection@0.9.0

## 0.8.0

### Minor Changes

- `conflicting-utilities` findings now explain the overlap. When the utilities are a shorthand and a longhand that it covers, such as `m-0 mb-4`, a note names the `allowShorthandOverrides` option. When a physical and a logical property overlap only in some writing modes, such as `px-2 pt-2`, a note names their logical group.

- When a physical utility conflicts with a logical utility, the `conflicting-utilities` finding shows how to use logical properties for both: it names a logical utility from your design system that writes the same value, such as `ms-4` for `ml-4` in `mx-auto ml-4`. When `writingModes` includes a vertical writing mode, as in the `stylistic` preset, it also names `pbs-2` for `pt-2` in `px-2 pt-2`. The suggestion uses the sides of horizontal left-to-right writing modes, appears only when `writingModes` includes `horizontal-tb`, and does not recommend logical properties over physical ones.

- Compiled code now imports its helpers from `stillwind/runtime`, or from `stillwind/runtime/development` in development builds. A module that uses several frameworks' macros imports each shared helper once. The `stillwind/runtime/class-object`, `stillwind/runtime/class-string`, `stillwind/runtime/class-name-object`, and matching `-development` entrypoints are removed. Recompile code that was compiled with an earlier version, because it imports these removed entrypoints.

- Add `style.when(variants)`, which stores a variant sequence such as `card("hover").in(list)`, `card.at("width >= 40rem")`, or `"hover:md"` in a variable. Use the variable as a computed key in `style()`, as a variant segment in a utility template, or in another `style.when()`, so several elements can share a ref condition without repeating it. Stored variants can be exported and imported. The new `invalid-stored-variant` lint rule reports an argument that is not a variant sequence and a use that can refer to several stored variants.

- Add the `or(...)` variant, which applies a utility when any of its comma-separated variant sequences matches. `or(hover,md:focus):p-4` means `hover:p-4 md:focus:p-4`, so each sequence keeps its own CSS order and lint findings. It works in utility strings, object keys of `style()` and `style.css()`, ref conditions such as `card("or(hover,focus-within)")`, and view-transition definitions. Design systems can declare the same behavior under another name with `@stillwind-variant name\(*\) (value: [variant-list]) { @stillwind-intrinsic builtin-or-variant; }`. `style.when()` now accepts several arguments and stores them as alternatives, which can include ref conditions and container queries, such as `style.when("hover", card("focus-within"))`.

- Export `StyleResultOf` from each framework entrypoint, such as `stillwind/react`. TypeScript can now emit declaration files for modules that export a `style()` result, for example with `declaration: true` or the `dts` option of `vp pack`. Before, this failed with TS2883 because the type of the result could not be named from a public entrypoint.

- `conflicting-utilities` has a `writingModes` option that lists the writing modes in which a physical utility and a logical utility are compared: `horizontal-tb`, `vertical-rl`, and `vertical-lr`. The default is `["horizontal-tb"]`, so `px-2 pt-2` is no longer reported, because the two utilities set different sides in horizontal text. `mx-auto ml-4` is still reported. The `stylistic` preset lists all three writing modes. Add `vertical-rl` or `vertical-lr` if your pages use vertical text.

### Patch Changes

- `conflicting-utilities` no longer reports a font size utility together with a `leading-*` utility, such as `text-sm leading-relaxed`. The font size utility reads the line height that the `leading-*` utility sets, so the result does not depend on their order.

- `ambiguous-style-base-order` and `interfering-imported-style-base` findings now include a note that reordering the `style()` arguments does not change which declaration wins. Fix these findings by building the overriding style on the other style or by removing the property from one of them.

- `conflicting-utilities` no longer reports a side utility together with a style utility that sets its style, such as `border-t border-dashed` or `divide-y divide-dashed`. The result of these pairs does not depend on their order.

- Updated dependencies:
  - @stillwind/config@0.8.0
  - @stillwind/native@0.8.0
  - @stillwind/projection@0.8.0

## 0.7.0

### Minor Changes

- `style.css()` now returns a minified stylesheet without the Stillwind and Tailwind license comments, so the strings in your bundle are smaller. The string still applies the same styles.

### Patch Changes

- Rules that a Tailwind CSS JavaScript plugin nests inside `@scope` within a selector, such as `@scope (&)`, now stay inside the `@scope` block, as the same rules in a CSS `@utility` do. Previously, each nested selector moved into the `@scope` prelude, so a selector that used `:scope` matched the document root instead of the scoping root and the rule never applied. For example, daisyUI's `join` lost the radius of its outer corners.

  A plugin that passes `addUtilities()` or `addComponents()` an `@scope` rule outside any selector, such as `{ "@scope (.theme)": { ".item": { … } } }`, now fails with an error. Previously, Stillwind generated CSS that styled other elements than Tailwind CSS does, without a diagnostic.

- Astro components whose frontmatter ends with a `//` line comment now pass `stillwind lint` and the build. Previously, such components failed with SW31005 ("Astro frontmatter coordinates do not match source"), so a trailing comment such as `// TODO` or a tool directive could fail the whole site build.

- The npm package page now shows that each package includes TypeScript declarations. Packages without a root JavaScript entry declare an empty root module, so TypeScript setups that ignore `exports` report a missing export instead of resolving unrelated types.

- Updated dependencies:
  - @stillwind/config@0.7.0
  - @stillwind/native@0.7.0
  - @stillwind/projection@0.7.0

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
