---
"stillwind": minor
---

`style.forward()` passes the class and style props that a component receives to the `style()` call of its element, as in `style("rounded p-4", style.forward({ className, style }))`. The forwarded classes come after the component's classes, and the forwarded inline style and custom properties replace the component's ones. A `style.cva()` factory takes a `style.forward()` value as its second argument. The React, Preact, Solid, Qwik, Vue, Svelte, and Astro entrypoints provide it.
