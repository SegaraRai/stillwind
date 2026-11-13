---
"stillwind": minor
---

Add the `ignoredUtilities` option to the `conflicting-utilities` lint rule. List utility name globs, such as `["btn"]`, and the rule stops reporting pairs that include a matching utility. Other pairs are still reported, so a component library base class such as daisyUI `btn` can be combined with `btn-outline` while `btn-outline btn-ghost` is still reported. Variants and the `!` modifier are ignored when the rule matches a name.
