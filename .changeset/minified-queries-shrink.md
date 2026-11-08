---
"@stillwind/native": patch
"@stillwind/integration": patch
---

`minifyCss()` now makes more rewrites that keep the meaning of the CSS:

- A numeric `calc()` in a custom property value or another unparsed value becomes a literal, as in `--x:calc(95 * 1%)` to `--x:95%` and `--y:calc(10px / 4)` to `--y:2.5px`. A division stays when its result does not terminate, such as `calc(1px / 3)`, and a unitless result stays when it is not an integer, unless `@property` registers the custom property as a `<number>`. A negative result also folds, so a property such as `width` that clamps a negative `calc()` result to zero rejects the folded literal instead.
- Media features use their boolean form when it matches the same devices, as in `(hover:hover)` to `(hover)`.
- A lower and an upper range of one feature become one interval, as in `(width>=40rem) and (width<60rem)` to `(40rem<=width<60rem)`. Browser targets without range syntax still receive `min-` and `max-` features.
- `@supports` declarations lose whitespace next to commas.
- A repeated selector in a selector list is removed, as in `.a,.b,.a` to `.a,.b`.
- Keyframes that have equal declarations are merged, as in `0%{opacity:0}50%{opacity:0}` to `0%,50%{opacity:0}`.
