# Ownership and string representation

Finite compiler decisions use enums. Parsers accept text at their boundaries and
return typed values after validation. Serialization preserves existing JSON values,
CSS spelling, generated names, and naming callback arguments.

Immutable snapshots, source revisions, project transactions, and queries that move
between threads retain their shared ownership. Evaluation records belong to one
evaluation and never become part of an immutable theme. JavaScript naming callbacks
use single-threaded shared ownership.

Immutable identifiers and names use `LeanStr`. Output buffers that append or replace
text use `String`. Borrowed source slices and externally supplied open identifiers
use `&str`. Conversion to JavaScript strings happens at the binding boundary.

## Implementation phases

1. Type generated symbol kinds, locator projections, container types, lint detection
   keys, canonical spacing units, viewport axes, and artifact adapter keys. Update
   binding declarations and callers while preserving their serialized values.
2. Store canonical labels directly in signature groups. Remove records and previews
   that no query reads after preparation. Reuse the analysis source in lint edits.
3. Move token and arbitrary-value recording into an evaluation context that borrows
   the immutable theme. Consume completed records without cloning their collections.
4. Use `Rc` for naming callbacks and generator sets. Preserve callback lifetime,
   pointer identity, memoization, and transaction behavior.
5. Adopt `LeanStr` for source identities, generated names, binding names, registry
   names, candidates, and canonical signatures where values remain immutable.
   Retain `String` for builders and mutable CSS or JavaScript output.

Each implementation phase is validated and committed independently. Final acceptance
includes the workspace check and test tasks, shared native/WASM behavior tests, and
native/WASM compiler benchmarks. Measurements compare the same benchmark cases and
record the runtime, representation limits, and any observed regressions.
