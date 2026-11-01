---
"@stillwind/vscode-extension": patch
"@stillwind/cli": patch
---

Changes to files that analysis never read no longer interrupt it. Other changes to closed files refresh analysis without restarting it, so editor features stay available during builds.
