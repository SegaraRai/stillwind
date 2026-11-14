---
"@stillwind/language-server": minor
"@stillwind/vscode-extension": minor
---

Editors now show hover previews for ref variants, such as `trigger("hover:")`, ``trigger`hover:` ``, `trigger.at("width >= 40rem")`, and `trigger.in(card)`, when they are used in `style()`. The preview shows the generated selector or container query of the variant, and it appears wherever the cursor is in the variant expression. Completion suggests variants in the text of a ref call or tag. In a utility template such as `` `${trigger("hover:")}:block` ``, the utility after the substitution now has a hover preview that includes the substituted variant.
