---
"@stillwind/vscode-extension": patch
---

The VS Code extension now loads a configuration only when an editor shows a file that uses it. Files that other extensions open in the background no longer start analysis processes or report configuration errors.
