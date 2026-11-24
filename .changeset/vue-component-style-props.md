---
"stillwind": patch
---

Vue templates can now pass `style()` to a component element, as in `<Button v-bind="style('p-4')">` or `<Button :class="style('p-4').class">`. Previously, the build failed with SW31009.
