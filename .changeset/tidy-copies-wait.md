---
"@stillwind/vscode-extension": patch
"@stillwind/cli": patch
---

Opening a file no longer loads a configuration from a dot directory inside the workspace, such as a generated `.e2e` copy of a project. To use such a configuration, set `stillwind.configFile`.
