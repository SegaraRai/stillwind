# Stillwind host orchestration

`@stillwind/host` shares CSS loading, semantic compilation, runtime finalization,
and JavaScript source mapping between bundler adapters. It does not own a global
runtime module graph. Build with `vp run --filter @stillwind/host build` before
using the package from a fresh checkout.

## Editor previews

`EditorProject(root, compilerHost, designSystem)` provides a versioned analysis
boundary for an editor or language-server transport. `designSystem` returns a
completed `NativeDesignSystemLoader`; each request creates its own native session.
No CSS asset is published and no bundler contribution registry is updated.

Call `update({ id, version, source })` for each unsaved document, then
`await analyze(id, abortSignal)`. Use absolute filesystem IDs matching the host
resolver. Versions must increase while a document remains open. Dependency reads
prefer the captured document overlays, including files not saved to disk.
`close(id)` removes the overlay. Call `invalidate()` when disk dependencies or
the design system change, and schedule analyses of the affected open documents.

Results have `coverage: 'observed'`, the captured project revision and document
version, dependencies, and one of four statuses:

- `success`: current compiled output, including an empty stylesheet after macro removal.
- `partial`: diagnostics and any compiler output available, or an analysis error.
- `stale`: a document, dependency notification, or design system changed during the request.
- `cancelled`: the request's abort signal was set.

Stale and cancelled results contain no current output. `previousSuccess` is
separately labelled with its own version and revision; it must not be presented
as the current document's successful analysis. Clients should also compare the
result revision/version at publication time. Cancellation is checked between
asynchronous reads; it cannot interrupt synchronous native compilation or the
TypeScript checker already executing in the same JavaScript turn.

This is an analysis API, not a complete LSP server or a completion provider.
Syntax recovery depends on the compiler: some malformed documents only produce
an error with a previous-success preview. Semantic dependency discovery follows
probe bindings through local declarations and requested barrel exports. Namespace
requests and declarations without an export slice retain a conservative fallback.
TypeScript can separately read additional imports and library declarations:
`checkerStats.overlayReads` and `checkerStats.diskReadRequests` count its overlay
reads and requests delegated to its filesystem implementation, respectively
(including unsuccessful reads). These are separate from the host's explicit
semantic `dependencies`.

Run `vp run --filter @stillwind/host test` for real native/checker verification of
unsaved imports, dependency edits, broken syntax, macro removal, cancellation,
and out-of-order completion.

## SFC type facts

The host resolves Stillwind identities with TypeScript 7 in JS/TS, Vue, Svelte and
Astro. Framework converters use their own TypeScript 6 SDK dependency. Install the
optional converter peers for the framework you use; versions and the coordinate
contract are documented in [SFC type checking](../../docs/sfc-type-checking.md).

`let selected = a; selected = b` can become incompatible after declaration identity
injection. Use `let selected: typeof a | typeof b = a` in TypeScript, or preserve
inference through a conditional/generic helper. The checker reports `SHTS002` for
identity-related type errors and `SHTS003` when an asset expression has lost finite
identity information. These are compilation errors, including in editor collect
mode. They cannot be suppressed with Stillwind lint policy.

`TypeFactSource.probes`, facts, asset identities and host dependency specifiers use
original UTF-8 byte ranges. `scanSource` provides the additional projected UTF-16
ranges needed by `TypeScriptFactsService`. Checker shadow text is never emitted;
the native compiler applies edits to the original SFC.
