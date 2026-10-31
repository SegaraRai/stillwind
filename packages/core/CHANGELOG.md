# stillwind

## 0.2.0

### Minor Changes

- The editor now finds the nearest `stillwind.config.*` from each opened file, so a monorepo works without a configuration file at its root.

- Reference configuration files, directories, and globs from `projects`, so each package in a monorepo can keep its own `stillwind.config.ts`.

- Import a file from `stillwind/logical/` after `stillwind` to make built-in axis utilities follow the writing direction, as `px-*` and `py-*` do. `overflow.css`, `overscroll.css`, `divide.css`, `snap.css`, and `resize.css` cover `overflow-x-*`, `overflow-y-*`, `overscroll-x-*`, `overscroll-y-*`, `divide-y-*`, `snap-x`, `snap-y`, `resize-x`, and `resize-y`. `prefer-logical-properties` no longer reports the `overflow`, `overscroll`, and `divide` utilities that these files change.

### Patch Changes

- `prefer-canonical-utilities` no longer replaces one theme token with another that only has the same value, such as `w-96` with `w-sm`. Fixed values now prefer spacing multiples, such as `w-112` for `w-[448px]`, and stay as written when several unrelated named tokens match.

- `prefer-logical-properties` no longer reports a shorthand that gives both axes the same value, such as `overflow-hidden` or `truncate`, and it no longer reports `sr-only` or `not-sr-only`, whose declarations you cannot change. `no-restricted-length-units` also skips `sr-only` by default.

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
