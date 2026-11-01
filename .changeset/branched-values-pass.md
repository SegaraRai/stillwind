---
"stillwind": patch
---

Arbitrary properties that use `--if()`, such as `[color:--if(--on,red,blue)]`, no longer report `incompatible-property-value` when the property accepts both branches. The rule still reports a branch that the property rejects.
