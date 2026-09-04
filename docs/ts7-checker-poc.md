# TypeScript 7 checker investigation

Status: promoted into the Vite compilation path against `typescript@7.0.2`.

## Finding

Stillwind can outsource TypeScript-aware candidate propagation to the TypeScript 7 Go checker.
The checked-in PoC demonstrates the complete transport loop: JavaScript client, spawned Go
process, virtual shadow source, project snapshot invalidation, cross-module type propagation, and
structured recovery of correlated `RefId`/`ScopeId` alternatives.

This is now Stillwind's primary typed propagation architecture.
The API entry points are exported under `typescript/unstable/*`, and the TypeScript team tracks the
broader API and content-mapper work for 7.1. Stillwind keeps a narrow language-neutral identity IR
at the checker boundary so the unstable transport remains replaceable.

There is a deliberate wording mismatch to account for: the 7.0 announcement says that 7.0 does
not ship a programmatic API, while the installed 7.0.2 artifact does export the working
`unstable/*` clients used here. The conservative interpretation is that the implementation is
available for experiments but carries no compatibility promise yet.

## Observed API surface in 7.0.2

The package exports synchronous and asynchronous clients. Both speak IPC to the bundled `tsgo`
binary. The synchronous client used by the PoC exposes:

- `API.updateSnapshot` and explicit changed-file invalidation;
- a virtual filesystem `readFile` callback with fallback to the real filesystem;
- `Program.getSourceFile`, source-file enumeration, and semantic diagnostics;
- `Checker.getTypeAtLocation`, `typeToString`, union inspection, symbols, and type arguments;
- request and server timing collection.

The API is curated rather than an in-process Go embedding surface. That boundary is favorable for
Rust: a production Rust adapter can own the child process and protocol, or a thin JavaScript host
can initially bridge it. Direct Rust-to-Go linking would couple Stillwind to non-public Go
internals and is not recommended.

The boundary is visibly still maturing. Recent upstream reports include a checker API panic on a
valid empty-tuple query and poor process-start error reporting in the synchronous client. Stillwind
therefore isolates and retries a failed sidecar once, then reports the integration failure. It does
not silently switch to a second, less capable propagation implementation.

## Proposed integration boundary

```text
Oxc extraction + resolved imports (Rust)
  -> declaration-site identities + shadow edits
  -> TS7 checker sidecar (IPC, long-lived per project)
  -> correlated candidate alternatives at queried use sites
  -> Stillwind symbolic IR (`Exact`, `Set`, `Universe`)
  -> CSS planning and runtime descriptors (Rust)
```

Oxc remains responsible for recognizing Stillwind calls, exact original spans, and JavaScript
support. TS7 is responsible only for propagation through TypeScript's module graph and type
system. Returned type facts must be converted immediately into Stillwind IR; compiler code should
not retain opaque checker objects across snapshots.

## Soundness rules

1. Treat missing, error, `any`, `unknown`, widened `string`, or unbranded alternatives as
   `Universe(kind)`, never as an empty candidate set.
2. Preserve each `Ref<RefId, ScopeId>` union constituent as one relation row. Never union the two
   axes independently.
3. Derive identities from normalized module identity and original callsite span, not shadow spans.
4. Reject or conservatively widen forged branded declarations outside generated/private types.
5. Keep package manifests for dependencies whose source is unavailable; the checker is not a
   substitute for Stillwind asset metadata.

## Implemented lifecycle

The Vite adapter owns one `TypeScriptFactsService` for its server/build lifetime. Its first request
opens the in-memory shadow project; later requests use `fileChanges` to create incremental
snapshots while retaining the same TS7 API process. The service tracks process starts, snapshots,
incremental snapshots, and restarts. A fixture updates one virtual source and verifies two
snapshots with one process start and one incremental update.

Snapshots remain request-scoped: checker objects are converted immediately to serializable facts,
the previous snapshot is disposed after its successor is ready, and no checker object crosses the
Rust boundary. A checker failure causes one cold-process retry. A second failure is surfaced; it
never activates a Rust implementation of TypeScript data flow.

The branded identity channel now covers refs, scopes, animations, and view transitions. Follow-up
hardening should export the TS7 timing counters (round trips, transferred bytes, and server time)
to Vite debug telemetry and add package-manifest inputs for dependencies without source.

## Primary sources

- [TypeScript 7.0 announcement](https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/)
- [TypeScript-Go API feature roadmap](https://github.com/microsoft/typescript-go/issues/4830)
- [TypeScript-Go API architecture discussion](https://github.com/microsoft/typescript-go/discussions/455)
- [Content mapper design discussion](https://github.com/microsoft/TypeScript/issues/63676)
- [Open `getTypeAtLocation` panic in 7.0.2](https://github.com/microsoft/typescript-go/issues/4804)
- [Open synchronous-client process error issue](https://github.com/microsoft/typescript-go/issues/4880)
