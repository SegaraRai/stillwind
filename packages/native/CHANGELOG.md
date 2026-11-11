# @stillwind/native

## 0.9.0

### Minor Changes

- Define scope variants with roots and limits, including nested scopes, and apply them in stylesheets and style calls.

### Patch Changes

- Keep valid pseudo-element styles while omitting nested child selectors that cannot match.

- Update Tailwind CSS compatibility to 4.3.5, preserve empty variable fallbacks, and improve group and peer selectors and Safari property fallbacks.

- Preserve color mixes without an explicit interpolation method without generating an ineffective fallback.

- `minifyCss()` now makes more rewrites that keep the meaning of the CSS:

  - A numeric `calc()` in a custom property value or another unparsed value becomes a literal, as in `--x:calc(95 * 1%)` to `--x:95%` and `--y:calc(10px / 4)` to `--y:2.5px`. A division stays when its result does not terminate, such as `calc(1px / 3)`, and a unitless result stays when it is not an integer, unless `@property` registers the custom property as a `<number>`. A negative result also folds, so a property such as `width` that clamps a negative `calc()` result to zero rejects the folded literal instead.
  - Media features use their boolean form when it matches the same devices, as in `(hover:hover)` to `(hover)`.
  - A lower and an upper range of one feature become one interval, as in `(width>=40rem) and (width<60rem)` to `(40rem<=width<60rem)`. Browser targets without range syntax still receive `min-` and `max-` features.
  - `@supports` declarations lose whitespace next to commas.
  - A repeated selector in a selector list is removed, as in `.a,.b,.a` to `.a,.b`.
  - Keyframes that have equal declarations are merged, as in `0%{opacity:0}50%{opacity:0}` to `0%,50%{opacity:0}`.

- Preserve scope boundaries and declaration order when compiling and minifying authored CSS.

- Keep pseudo-elements outside group and peer selectors, so variants such as `before:group-hover:flex` match in style calls, `@apply`, and nested `@variant` blocks. Custom wrapper variants such as `:is(.theme>&)` now also compose with `group-*` and `peer-*` without spaces around the combinator.

- Preserve fractional opacity and theme alpha values when suggesting canonical class names.

- `minifyCss()` now removes whitespace next to commas in values that Lightning CSS does not parse, such as custom property values and values that contain `var()`. Inside such a `calc()` or other math function, it also removes whitespace next to `*` and `/`. For example, `calc(var(--spacing) * 4)` becomes `calc(var(--spacing)*4)`, so minified CSS is smaller.

- Updated dependencies:
  - @stillwind/native-darwin-arm64@0.9.0
  - @stillwind/native-darwin-x64@0.9.0
  - @stillwind/native-linux-arm64-gnu@0.9.0
  - @stillwind/native-linux-x64-gnu@0.9.0
  - @stillwind/native-wasm32-wasi@0.9.0
  - @stillwind/native-win32-arm64-msvc@0.9.0
  - @stillwind/native-win32-x64-msvc@0.9.0

## 0.8.0

### Patch Changes

- Updated dependencies:
  - @stillwind/native-darwin-arm64@0.8.0
  - @stillwind/native-darwin-x64@0.8.0
  - @stillwind/native-linux-arm64-gnu@0.8.0
  - @stillwind/native-linux-x64-gnu@0.8.0
  - @stillwind/native-wasm32-wasi@0.8.0
  - @stillwind/native-win32-arm64-msvc@0.8.0
  - @stillwind/native-win32-x64-msvc@0.8.0

## 0.7.0

### Patch Changes

- Updated dependencies:
  - @stillwind/native-darwin-arm64@0.7.0
  - @stillwind/native-darwin-x64@0.7.0
  - @stillwind/native-linux-arm64-gnu@0.7.0
  - @stillwind/native-linux-x64-gnu@0.7.0
  - @stillwind/native-wasm32-wasi@0.7.0
  - @stillwind/native-win32-arm64-msvc@0.7.0
  - @stillwind/native-win32-x64-msvc@0.7.0

## 0.6.0

### Patch Changes

- Updated dependencies:
  - @stillwind/native-darwin-arm64@0.6.0
  - @stillwind/native-darwin-x64@0.6.0
  - @stillwind/native-linux-arm64-gnu@0.6.0
  - @stillwind/native-linux-x64-gnu@0.6.0
  - @stillwind/native-wasm32-wasi@0.6.0
  - @stillwind/native-win32-arm64-msvc@0.6.0
  - @stillwind/native-win32-x64-msvc@0.6.0

## 0.5.0

### Patch Changes

- Updated dependencies:
  - @stillwind/native-darwin-arm64@0.5.0
  - @stillwind/native-darwin-x64@0.5.0
  - @stillwind/native-linux-arm64-gnu@0.5.0
  - @stillwind/native-linux-x64-gnu@0.5.0
  - @stillwind/native-wasm32-wasi@0.5.0
  - @stillwind/native-win32-arm64-msvc@0.5.0
  - @stillwind/native-win32-x64-msvc@0.5.0

## 0.4.1

### Patch Changes

- Updated dependencies:
  - @stillwind/native-darwin-arm64@0.4.1
  - @stillwind/native-darwin-x64@0.4.1
  - @stillwind/native-linux-arm64-gnu@0.4.1
  - @stillwind/native-linux-x64-gnu@0.4.1
  - @stillwind/native-wasm32-wasi@0.4.1
  - @stillwind/native-win32-arm64-msvc@0.4.1
  - @stillwind/native-win32-x64-msvc@0.4.1

## 0.4.0

### Patch Changes

- Updated dependencies:
  - @stillwind/native-darwin-arm64@0.4.0
  - @stillwind/native-darwin-x64@0.4.0
  - @stillwind/native-linux-arm64-gnu@0.4.0
  - @stillwind/native-linux-x64-gnu@0.4.0
  - @stillwind/native-wasm32-wasi@0.4.0
  - @stillwind/native-win32-arm64-msvc@0.4.0
  - @stillwind/native-win32-x64-msvc@0.4.0

## 0.3.0

### Patch Changes

- Updated dependencies:
  - @stillwind/native-darwin-arm64@0.3.0
  - @stillwind/native-darwin-x64@0.3.0
  - @stillwind/native-linux-arm64-gnu@0.3.0
  - @stillwind/native-linux-x64-gnu@0.3.0
  - @stillwind/native-wasm32-wasi@0.3.0
  - @stillwind/native-win32-arm64-msvc@0.3.0
  - @stillwind/native-win32-x64-msvc@0.3.0

## 0.2.0

### Patch Changes

- Updated dependencies:
  - @stillwind/native-darwin-arm64@0.2.0
  - @stillwind/native-darwin-x64@0.2.0
  - @stillwind/native-linux-arm64-gnu@0.2.0
  - @stillwind/native-linux-x64-gnu@0.2.0
  - @stillwind/native-wasm32-wasi@0.2.0
  - @stillwind/native-win32-arm64-msvc@0.2.0
  - @stillwind/native-win32-x64-msvc@0.2.0

## 0.1.0

### Patch Changes

- Updated dependencies:
  - @stillwind/native-darwin-arm64@0.1.0
  - @stillwind/native-darwin-x64@0.1.0
  - @stillwind/native-linux-arm64-gnu@0.1.0
  - @stillwind/native-linux-x64-gnu@0.1.0
  - @stillwind/native-wasm32-wasi@0.1.0
  - @stillwind/native-win32-arm64-msvc@0.1.0
  - @stillwind/native-win32-x64-msvc@0.1.0
