# TypeScript 7 shadow-source checker PoC

This experiment tests whether Stillwind can delegate cross-module handle propagation to the
Go implementation of the TypeScript checker without writing generated files to disk.

Run it from the repository root:

```sh
vp run ts7-shadow:poc
```

The runner uses the unstable synchronous API shipped in `typescript@7.0.2`. It opens the real
fixture project, finds `style.scope()` and `style.ref()` callsites in the TS7 AST, injects a
declaration-site string-literal generic/phantom argument into an in-memory shadow copy, invalidates those files,
and asks a second TS7 snapshot for the propagated types. The output lists the ref/scope identity
pairs recovered after exports, re-exports, aliases, a conditional, and a generic identity call.

No fixture source is modified. Identity offsets are positions in the original source, so inserted
generic syntax cannot destabilize identities later in the file.

## What this proves

- The public npm artifact can spawn the Go checker over IPC from JavaScript.
- Virtual filesystem callbacks are sufficient for a memory-only shadow source.
- Literal identities flow through normal TypeScript module and type inference.
- Keeping `Ref<RefId, ScopeId>` as each union constituent preserves ref/scope correlation.
- The checker API exposes the operations Stillwind needs here: snapshots, AST traversal,
  `getTypeAtLocation`, union constituents, symbols, type arguments, and type rendering.

## What it does not prove

- `typescript/unstable/*` is not a stable integration contract.
- The producer recognizer only proves the shadow/checker loop. Production recognition must use
  Stillwind's resolved Oxc bindings and provide edits plus original/shadow span mapping.
- TS types are erased in JavaScript and can be widened by `any`, assertions, untyped dependencies,
  or deliberately misleading declarations. The Rust compiler still needs sound fallback behavior.
- TS7 content mappers are not part of this PoC. Their 7.1 design is aimed primarily at non-TS
  embedded languages; the virtual-FS callback already covers this TS-to-TS shadowing experiment.
