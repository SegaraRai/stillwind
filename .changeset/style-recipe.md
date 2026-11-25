---
"stillwind": minor
---

`style.cva()` is now `style.recipe()`. The configuration and the returned factory do not change. Replace each `style.cva(` call with `style.recipe(`. The lint rules `invalid-cva-config` and `invalid-cva-input` are now `invalid-recipe-config` and `invalid-recipe-input`, so rename them in your lint configuration too. The old names are no longer accepted.
