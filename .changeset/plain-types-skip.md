---
"@stillwind/cli": patch
"@stillwind/vscode-extension": patch
---

Declaration files such as `vite-env.d.ts` that match a `*.ts` glob in `files` no longer make `stillwind lint` fail or show an analysis error in the editor.
