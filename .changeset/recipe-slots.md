---
"stillwind": minor
---

`style.recipe()` accepts `slots` and `compoundSlots` in the format of tailwind-variants. With `slots`, the factory returns one function for each slot, such as `const { base, icon } = card({ size })`, and each function returns props for that element. Option values and compound `class` values can set styles for several slots with an object such as `{ base: "p-2", icon: "size-3" }`. A slot function accepts its own classes, option values for that slot only, and a `style.forward()` value.
