---
"stillwind": minor
---

`conflicting-utilities` has a `writingModes` option that lists the writing modes in which a physical utility and a logical utility are compared: `horizontal-tb`, `vertical-rl`, and `vertical-lr`. The default is `["horizontal-tb"]`, so `px-2 pt-2` is no longer reported, because the two utilities set different sides in horizontal text. `mx-auto ml-4` is still reported. The `stylistic` preset lists all three writing modes. Add `vertical-rl` or `vertical-lr` if your pages use vertical text.
