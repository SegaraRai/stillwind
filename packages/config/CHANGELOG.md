# @stillwind/config

## 0.10.0

### Patch Changes

- Updated dependencies:
  - @stillwind/native@0.10.0
  - @stillwind/projection@0.10.0

## 0.9.0

### Minor Changes

- Enumerate the target files of a project with `listProjectFiles` from `@stillwind/config/node`. `resolveIntegrationConfig` now also returns the `scope` of the selected project, which holds its root and its `files` and `exclude` globs.

- Generated utility class names now stay the same when an edit elsewhere in the module moves the `style()` call. A class name also no longer changes when only the generated name of an imported ref or container changes.

  The `utilityClass` and `viewTransitionSlot` naming callbacks now receive `path` and `ordinal` instead of `call`. `path` lists the names of the declarations that enclose the `style()` call, from the outermost to the innermost, and `ordinal` counts the earlier calls of the module with the same path.

- Generated names of refs, anchors, containers, keyframes, and view transitions now stay the same when an edit elsewhere in the module moves the declaration. Type facts of consumers also stay the same, so such an edit in a dependency no longer changes the types that its consumers see.

  Naming callbacks for these names now receive `path` and `ordinal` instead of `start` and `end`. `path` lists the names of the declarations that enclose the declaration, from the outermost to the innermost. `ordinal` counts the earlier declarations of the module with the same kind, path, and binding name. Composition identities and asset identities in compilation results carry `path` and `ordinal` instead of byte offsets.

### Patch Changes

- Each published npm package and the VS Code extension now include `CHANGELOG.md`, so you can read the release notes of the installed version in `node_modules` or on the extension page.

- Updated dependencies:
  - @stillwind/native@0.9.0
  - @stillwind/projection@0.9.0

## 0.8.0

### Patch Changes

- Updated dependencies:
  - @stillwind/native@0.8.0
  - @stillwind/projection@0.8.0

## 0.7.0

### Patch Changes

- The npm package page now shows that each package includes TypeScript declarations. Packages without a root JavaScript entry declare an empty root module, so TypeScript setups that ignore `exports` report a missing export instead of resolving unrelated types.

- Updated dependencies:
  - @stillwind/native@0.7.0
  - @stillwind/projection@0.7.0

## 0.6.0

### Patch Changes

- Updated dependencies:
  - @stillwind/native@0.6.0
  - @stillwind/projection@0.6.0

## 0.5.0

### Patch Changes

- Updated dependencies:
  - @stillwind/native@0.5.0
  - @stillwind/projection@0.5.0

## 0.4.1

### Patch Changes

- Updated dependencies:
  - @stillwind/native@0.4.1
  - @stillwind/projection@0.4.1

## 0.4.0

### Patch Changes

- Updated dependencies:
  - @stillwind/native@0.4.0
  - @stillwind/projection@0.4.0

## 0.3.0

### Patch Changes

- Updated dependencies:
  - @stillwind/native@0.3.0
  - @stillwind/projection@0.3.0

## 0.2.0

### Minor Changes

- Reference configuration files, directories, and globs from `projects`, so each package in a monorepo can keep its own `stillwind.config.ts`.

### Patch Changes

- Updated dependencies:
  - @stillwind/native@0.2.0
  - @stillwind/projection@0.2.0

## 0.1.0

### Patch Changes

- Updated dependencies:
  - @stillwind/native@0.1.0
  - @stillwind/projection@0.1.0
