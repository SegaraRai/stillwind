# @stillwind/host

## 0.9.0

### Minor Changes

- Vite builds and development servers now run TypeScript type collection once for all environments, such as client and SSR, while the environments resolve imports of source files to the same files. This shortens builds and the first page load. Packages may resolve differently, for example when SSR externalizes them. An environment that resolves an import of a source file differently continues with its own type collection.

  Hosts that share a `CompilationProject` can set `resolverDomain` on `CompilerHost` to name their resolver. When another domain resolves an import of a source file differently, preparation and compilation reject with `ResolutionMismatchError`. Prepared import links no longer point the checker at package runtime files, so the checker resolves package types itself.

- Generated utility class names now stay the same when an edit elsewhere in the module moves the `style()` call. A class name also no longer changes when only the generated name of an imported ref or container changes.

  The `utilityClass` and `viewTransitionSlot` naming callbacks now receive `path` and `ordinal` instead of `call`. `path` lists the names of the declarations that enclose the `style()` call, from the outermost to the innermost, and `ordinal` counts the earlier calls of the module with the same path.

- Generated names of refs, anchors, containers, keyframes, and view transitions now stay the same when an edit elsewhere in the module moves the declaration. Type facts of consumers also stay the same, so such an edit in a dependency no longer changes the types that its consumers see.

  Naming callbacks for these names now receive `path` and `ordinal` instead of `start` and `end`. `path` lists the names of the declarations that enclose the declaration, from the outermost to the innermost. `ordinal` counts the earlier declarations of the module with the same kind, path, and binding name. Composition identities and asset identities in compilation results carry `path` and `ordinal` instead of byte offsets.

### Patch Changes

- Compile modules that import one style from a module whose other styles compose bases from further modules. These builds no longer fail with `SW11003: style base producer is unavailable` or with an undetermined declaration identity.

- Apply edits of keyframes and view-transition declarations without compiling the modules that use them again. The document stylesheet now takes these declarations from the module that declares them.

- Accept modern CSS length units in typed Tailwind plugins and preserve DEFAULT entries when reading theme namespaces.

- Updated dependencies:
  - @stillwind/config@0.9.0
  - @stillwind/native@0.9.0
  - @stillwind/projection@0.9.0

## 0.8.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.8.0
  - @stillwind/native@0.8.0
  - @stillwind/projection@0.8.0

## 0.7.0

### Patch Changes

- The npm package page now shows that each package includes TypeScript declarations. Packages without a root JavaScript entry declare an empty root module, so TypeScript setups that ignore `exports` report a missing export instead of resolving unrelated types.

- Updated dependencies:
  - @stillwind/config@0.7.0
  - @stillwind/native@0.7.0
  - @stillwind/projection@0.7.0

## 0.6.0

### Patch Changes

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

- Updated dependencies:
  - @stillwind/config@0.4.1
  - @stillwind/native@0.4.1
  - @stillwind/projection@0.4.1

## 0.4.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.4.0
  - @stillwind/native@0.4.0
  - @stillwind/projection@0.4.0

## 0.3.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.3.0
  - @stillwind/native@0.3.0
  - @stillwind/projection@0.3.0

## 0.2.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.2.0
  - @stillwind/native@0.2.0
  - @stillwind/projection@0.2.0

## 0.1.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.1.0
  - @stillwind/native@0.1.0
  - @stillwind/projection@0.1.0
