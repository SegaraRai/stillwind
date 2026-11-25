---
"stillwind": minor
---

The new `utility-class-selector` lint rule reports variant selectors that test the `class` attribute for a utility name, such as `[&_svg:not([class*='size-'])]:size-4` in shadcn/ui components or `[&.border-b]:pb-6`. Stillwind writes generated class names, so these tests do not see the utilities of an element, and a default such as the icon size replaces the size that the element sets. For the default-size form, the automatic fix gives the rule zero specificity with `:where()`, such as `[:where(&_svg)]:size-4` or `[:where(&>svg)]:size-8` for `*:[svg:not([class*=size-])]:size-8`, so a class on the element that sets the same property wins.
