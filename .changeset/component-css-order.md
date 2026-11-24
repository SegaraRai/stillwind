---
"stillwind": minor
---

When a file passes a `style()` result to a component that it imports, the CSS of the file that declares the component now comes first, so the caller's utilities win over the component's own utilities. Previously, files without a base relationship were ordered by path, so a style passed from `src/App.tsx` could lose to a component in `src/components/`. The same order applies when a file passes an imported value directly to `style()`. The new `cyclic-style-order` lint rule reports files that pass styles to components of each other.
