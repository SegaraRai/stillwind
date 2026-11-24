---
"stillwind": patch
---

A `style()` call that is the body of an arrow function, such as `(className) => style("p-4", { class: className })` or Vue's `computed(() => style("p-4"))`, now compiles to valid JavaScript. Previously, the compiled object literal was read as a block, so the build failed.
