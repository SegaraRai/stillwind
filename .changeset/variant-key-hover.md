---
"@stillwind/language-server": minor
"@stillwind/vscode-extension": minor
---

Editors now show hover previews and completions for variant keys, such as `"hover:"` in `style({ "hover:": "block" })`, including nested keys and the keys in `style.css()` and `style.cva()` inputs. The arguments of `style.when()` get the same hover previews and completions. A hover preview shows the selectors and conditions that the variants add, including the variants of enclosing keys. Completion suggests only variants, also while the key is still incomplete.
