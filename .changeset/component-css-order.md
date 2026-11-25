---
"stillwind": minor
---

When a file imports and uses a value from another file, such as a component, the CSS of the file that declares the value now comes first. A style that `src/App.tsx` passes to a component therefore wins over the component's own utilities, whether the style is written in the element's props, stored in a variable, or passed to a child element as with `asChild`. Previously, files without a base relationship were ordered by path, so such a style could lose to a component in `src/components/`. Imports through re-exporting files and through files without `style()` calls order files in the same way. The new `cyclic-style-order` lint rule reports files that import values from each other, so that both orders cannot hold.
