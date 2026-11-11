---
"@stillwind/config": minor
"@stillwind/host": minor
---

Generated names of refs, anchors, containers, keyframes, and view transitions now stay the same when an edit elsewhere in the module moves the declaration. Type facts of consumers also stay the same, so such an edit in a dependency no longer changes the types that its consumers see.

Naming callbacks for these names now receive `path` and `ordinal` instead of `start` and `end`. `path` lists the names of the declarations that enclose the declaration, from the outermost to the innermost. `ordinal` counts the earlier declarations of the module with the same kind, path, and binding name. Composition identities and asset identities in compilation results carry `path` and `ordinal` instead of byte offsets.
