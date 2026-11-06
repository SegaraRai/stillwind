---
"stillwind": minor
---

Compiled code now imports its helpers from `stillwind/runtime`, or from `stillwind/runtime/development` in development builds. A module that uses several frameworks' macros imports each shared helper once. The `stillwind/runtime/class-object`, `stillwind/runtime/class-string`, `stillwind/runtime/class-name-object`, and matching `-development` entrypoints are removed. Recompile code that was compiled with an earlier version, because it imports these removed entrypoints.
