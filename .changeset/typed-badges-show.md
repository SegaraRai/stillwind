---
"stillwind": patch
"@stillwind/analysis": patch
"@stillwind/cli": patch
"@stillwind/config": patch
"@stillwind/host": patch
"@stillwind/integration": patch
"@stillwind/language-server": patch
"@stillwind/projection": patch
"@stillwind/vscode-extension": patch
"prettier-plugin-stillwind": patch
---

The npm package page now shows that each package includes TypeScript declarations. Packages without a root JavaScript entry declare an empty root module, so TypeScript setups that ignore `exports` report a missing export instead of resolving unrelated types.
