---
"@stillwind/config": minor
"@stillwind/host": minor
---

Generated utility class names now stay the same when an edit elsewhere in the module moves the `style()` call. A class name also no longer changes when only the generated name of an imported ref or container changes.

The `utilityClass` and `viewTransitionSlot` naming callbacks now receive `path` and `ordinal` instead of `call`. `path` lists the names of the declarations that enclose the `style()` call, from the outermost to the innermost, and `ordinal` counts the earlier calls of the module with the same path.
