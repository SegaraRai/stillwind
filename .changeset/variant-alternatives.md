---
"stillwind": minor
---

Add the `or(...)` variant, which applies a utility when any of its comma-separated variant sequences matches. `or(hover,md:focus):p-4` means `hover:p-4 md:focus:p-4`, so each sequence keeps its own CSS order and lint findings. It works in utility strings, object keys of `style()` and `style.css()`, ref conditions such as `card("or(hover,focus-within)")`, and view-transition definitions. Design systems can declare the same behavior under another name with `@stillwind-variant name\(*\) (value: [variant-list]) { @stillwind-intrinsic builtin-or-variant; }`. `style.when()` now accepts several arguments and stores them as alternatives, which can include ref conditions and container queries, such as `style.when("hover", card("focus-within"))`.
