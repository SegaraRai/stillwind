---
"@stillwind/vscode-extension": patch
"@stillwind/cli": patch
---

File changes in `node_modules`, `target`, and dot directories no longer interrupt analysis. Other changes to closed files refresh analysis without restarting it, so editor features stay available during builds.
