---
"stillwind": minor
---

Add `style.when(variants)`, which stores a variant sequence such as `card("hover").in(list)`, `card.at("width >= 40rem")`, or `"hover:md"` in a variable. Use the variable as a computed key in `style()`, as a variant segment in a utility template, or in another `style.when()`, so several elements can share a ref condition without repeating it. Stored variants can be exported and imported. The new `invalid-stored-variant` lint rule reports an argument that is not a variant sequence and a use that can refer to several stored variants.
