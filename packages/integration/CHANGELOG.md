# @stillwind/integration

## 0.10.1

### Patch Changes

- Vite development servers now apply edits of style modules that an editor or tool saves by truncating the file and then writing it. Before this fix, a hot update on Linux could read the file while it was empty, and the edited styles stayed missing until the next edit.

- Vite development servers now reload CSS for every file that is added to a directory that a CSS plugin lists, such as an icon directory. Before this fix, a file that was created while the server started or while the design system reloaded could be missed until the next change in that directory.

- Updated dependencies:
  - @stillwind/config@0.10.1
  - @stillwind/host@0.10.1
  - @stillwind/native@0.10.1
  - stillwind@0.10.1

## 0.10.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.10.0
  - @stillwind/host@0.10.0
  - @stillwind/native@0.10.0
  - stillwind@0.10.0

## 0.9.0

### Patch Changes

- Build Vite applications faster when pages reach source files only through dynamic imports, such as an `index.html` entry with lazy routes or Astro pages. A build now loads the source files that the project's `files` globs select into the type checker before the first compilation, so the checker no longer rebuilds its project for each newly found page.

- Start Vite development servers and apply edits of shared style modules faster. A development server now plans the document stylesheet once after a burst of compilations instead of after each compilation.

- Apply edits of a shared module faster in the Vite development server. A module that imports the edited module receives a hot update only when its transformed JavaScript changes.

- Apply edits of a shared module faster in the Vite development server. When an edit keeps the class names and types that a module exports, for example when it only changes utilities, the modules that import it are not compiled again. Their checker diagnostics then update when they are compiled for another reason.

- Start Vite development servers and apply edits faster in projects whose modules share styles. The development server now prepares the files that the build inputs reach once, and compilations share their scans and checker results until a file changes.

- Load the first page of a Vite development server faster. The server now starts the type checker when it starts, instead of at the first request that compiles a module.

- Update document CSS faster after an edit in Next and Turbopack development. Document CSS reconstruction now reuses the imports of modules whose source is unchanged and reads each level of the module graph concurrently, and Next route discovery reads route directories concurrently. Adding or removing a file still makes reconstruction resolve every import again.

- Start Vite development servers faster in large projects. While no browser is connected, compilations no longer plan the document stylesheet every 200 milliseconds, so the first server-rendered page of a cold start is ready sooner.

- Fix document CSS in large Next applications that use webpack. After an edit, the stylesheet failed with "Maximum call stack size exceeded" while its source map was read, so the page returned an error and the edit never applied.

- Keep Next.js development CSS in sync when routes are added or removed during stylesheet compilation.

- `minifyCss()` now makes more rewrites that keep the meaning of the CSS:

  - A numeric `calc()` in a custom property value or another unparsed value becomes a literal, as in `--x:calc(95 * 1%)` to `--x:95%` and `--y:calc(10px / 4)` to `--y:2.5px`. A division stays when its result does not terminate, such as `calc(1px / 3)`, and a unitless result stays when it is not an integer, unless `@property` registers the custom property as a `<number>`. A negative result also folds, so a property such as `width` that clamps a negative `calc()` result to zero rejects the folded literal instead.
  - Media features use their boolean form when it matches the same devices, as in `(hover:hover)` to `(hover)`.
  - A lower and an upper range of one feature become one interval, as in `(width>=40rem) and (width<60rem)` to `(40rem<=width<60rem)`. Browser targets without range syntax still receive `min-` and `max-` features.
  - `@supports` declarations lose whitespace next to commas.
  - A repeated selector in a selector list is removed, as in `.a,.b,.a` to `.a,.b`.
  - Keyframes that have equal declarations are merged, as in `0%{opacity:0}50%{opacity:0}` to `0%,50%{opacity:0}`.

- Update document CSS faster after an edit in Next and Turbopack development. When an edited module keeps its exports, the modules that import it are no longer compiled again, and their checker diagnostics are refreshed when they are compiled for another reason. Next with webpack also renders the document stylesheet once for each change instead of once for each compiler and layer.

- Build and edit Next applications faster. The Stillwind loaders for Turbopack and webpack in Next now reuse the evaluated configuration, the design system, and the type checker across modules in one process while the configuration and CSS files stay unchanged. Document CSS is compiled again only for the modules whose sources or imported modules changed since the previous reconstruction in the same process.

- Each published npm package and the VS Code extension now include `CHANGELOG.md`, so you can read the release notes of the installed version in `node_modules` or on the extension page.

- Start the React Router development server faster. Stillwind no longer compiles routes in the child compiler that React Router uses to read route exports during development, as it already did in builds.

- Vite builds and development servers now run TypeScript type collection once for all environments, such as client and SSR, while the environments resolve imports of source files to the same files. This shortens builds and the first page load. Packages may resolve differently, for example when SSR externalizes them. An environment that resolves an import of a source file differently continues with its own type collection.

  Hosts that share a `CompilationProject` can set `resolverDomain` on `CompilerHost` to name their resolver. When another domain resolves an import of a source file differently, preparation and compilation reject with `ResolutionMismatchError`. Prepared import links no longer point the checker at package runtime files, so the checker resolves package types itself.

- Production Vite builds compile faster. Compilations in one build now share scanned sources, import resolutions, and type facts, and React Router builds no longer compile styles for route-export scans and client route stubs.

- Build Next applications with Turbopack faster. A production build now loads the entries and the source files that the project's `files` globs select into one type checker that the loader and document CSS compilers of a worker process share, so the checker no longer rebuilds its project for each compiled module.

- Apply edits faster in Next development servers that use Turbopack. The Stillwind loaders of a worker process now keep the project files in one type checker between compilations and watch the application directory for changes, so an edit no longer reloads the whole project into the checker.

- Compile modules faster when they import other style modules. A compilation no longer builds output again for imported modules that did not change.

- Build Vue, Svelte, and Astro projects faster. A module that several entries import is no longer compiled again for each entry while its source and the facts that it reads stay unchanged, and component files now receive the same stylesheet imports as a dependency and as an entry.

- Compile faster when many modules import the same styles. A compilation no longer rebuilds the dependencies whose source did not change, and development servers no longer render the whole stylesheet again when a compilation leaves its producers unchanged. Editing a shared style module in a Vite development server now applies the change sooner.

- `minifyCss()` now removes whitespace next to commas in values that Lightning CSS does not parse, such as custom property values and values that contain `var()`. Inside such a `calc()` or other math function, it also removes whitespace next to `*` and `/`. For example, `calc(var(--spacing) * 4)` becomes `calc(var(--spacing)*4)`, so minified CSS is smaller.

- Build webpack and Rspack applications faster with `StillwindPlugin`. The producer loaders of a compiler now share one design system, compiler session, and type checker while the configuration and CSS files stay unchanged, and document CSS reconstruction reuses their compiled modules instead of compiling every producer again. In watch mode, the shared checker receives the files that webpack or Rspack reports as changed before each compilation.

- Updated dependencies:
  - @stillwind/config@0.9.0
  - @stillwind/host@0.9.0
  - @stillwind/native@0.9.0
  - stillwind@0.9.0

## 0.8.0

### Patch Changes

- A React Router production build no longer fails at random with `Server-only module referenced by client` for a route that imports a `.server` module with `?stillwind.css`, or whose styles depend on a `.server` module. Rolldown could pass the option that Stillwind uses to skip React Router's check to a different resolution that runs at the same time. Stillwind now resolves such an import again when React Router reports this error. Imports that remain in client code are still checked.

- Updated dependencies:
  - @stillwind/config@0.8.0
  - @stillwind/host@0.8.0
  - @stillwind/native@0.8.0
  - stillwind@0.8.0

## 0.7.0

### Patch Changes

- Astro sites that use the ClientRouter now keep Stillwind styles from client scripts after a client-side navigation. Previously, the stylesheet that a client script loaded was removed from the page head when the router swapped pages, so elements that the script created stayed unstyled until a full reload.

- Rules that a Tailwind CSS JavaScript plugin nests inside `@scope` within a selector, such as `@scope (&)`, now stay inside the `@scope` block, as the same rules in a CSS `@utility` do. Previously, each nested selector moved into the `@scope` prelude, so a selector that used `:scope` matched the document root instead of the scoping root and the rule never applied. For example, daisyUI's `join` lost the radius of its outer corners.

  A plugin that passes `addUtilities()` or `addComponents()` an `@scope` rule outside any selector, such as `{ "@scope (.theme)": { ".item": { … } } }`, now fails with an error. Previously, Stillwind generated CSS that styled other elements than Tailwind CSS does, without a diagnostic.

- A SvelteKit server that uses `@stillwind/integration/sveltekit/handle` now starts when the package is installed from npm. Previously, the SSR build left the handle as an external import, so a production server, for example one built with `@sveltejs/adapter-node`, failed to start with `Cannot find package '$app'`. You can remove an `ssr.noExternal` entry that you added for `@stillwind/integration` as a workaround.

- Astro components whose frontmatter ends with a `//` line comment now pass `stillwind lint` and the build. Previously, such components failed with SW31005 ("Astro frontmatter coordinates do not match source"), so a trailing comment such as `// TODO` or a tool directive could fail the whole site build.

- The npm package page now shows that each package includes TypeScript declarations. Packages without a root JavaScript entry declare an empty root module, so TypeScript setups that ignore `exports` report a missing export instead of resolving unrelated types.

- Updated dependencies:
  - @stillwind/config@0.7.0
  - @stillwind/host@0.7.0
  - @stillwind/native@0.7.0
  - stillwind@0.7.0

## 0.6.0

### Patch Changes

- Vite-based integrations, including Astro, now warn when application code or a framework option such as Starlight's `customCss` imports the CSS entrypoint selected in `stillwind.config.ts`. The integration already delivers the compiled CSS, and importing the entrypoint also sends its uncompiled `@theme` and `@utility` rules to the page. Remove the import to fix the warning.

- In development, pages that link `/__stillwind/style.css?environment=ssr` now reload that stylesheet when the CSS changes. Theme edits now reach pages that never load the Stillwind client runtime, such as resumable Qwik pages, without importing the CSS entrypoint or reloading the page.

- The packages now have npm keywords, so the npm search, the Astro integrations library, and other directories that read `keywords` can find them by the frameworks, build tools, and editors that they support. Each package description now states what the package does and which tools it supports.

- Updated dependencies:
  - @stillwind/config@0.6.0
  - @stillwind/host@0.6.0
  - @stillwind/native@0.6.0
  - stillwind@0.6.0

## 0.5.0

### Patch Changes

- During Vite development, server rendering now shows a component edit even when a framework transforms the edited file for the server before the hot update runs. Previously, TanStack Start could keep rendering the component from before the edit until the server restarted.

- Updated dependencies:
  - @stillwind/config@0.5.0
  - @stillwind/host@0.5.0
  - @stillwind/native@0.5.0
  - stillwind@0.5.0

## 0.4.1

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.4.1
  - @stillwind/host@0.4.1
  - @stillwind/native@0.4.1
  - stillwind@0.4.1

## 0.4.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.4.0
  - @stillwind/host@0.4.0
  - @stillwind/native@0.4.0
  - stillwind@0.4.0

## 0.3.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.3.0
  - @stillwind/host@0.3.0
  - @stillwind/native@0.3.0
  - stillwind@0.3.0

## 0.2.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.2.0
  - @stillwind/host@0.2.0
  - @stillwind/native@0.2.0
  - stillwind@0.2.0

## 0.1.0

### Patch Changes

- Updated dependencies:
  - @stillwind/config@0.1.0
  - @stillwind/host@0.1.0
  - @stillwind/native@0.1.0
  - stillwind@0.1.0
