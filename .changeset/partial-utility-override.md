---
"stillwind": minor
---

The new `partial-utility-override` lint rule reports a utility that overrides another utility of the same family but leaves some of its properties in place, such as `text-[0.8rem]` over `text-sm`, which keeps the line height of `text-sm`. tailwind-merge removes the whole overridden utility, so code migrated from `cn()` can look different. The rule checks utilities added to a base with `style(base, ...)`, utilities under a JavaScript condition, and variant options and compound values of `style.recipe()`.

`conflicting-utilities` no longer reports a utility under a JavaScript condition that is written after the other utility, as in `style("p-2", wide && "p-4")`, because the conditional utility overrides the other one when the condition is true, as the code shows.
