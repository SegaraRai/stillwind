---
"stillwind": patch
"@stillwind/vscode-extension": patch
"@stillwind/cli": patch
---

The Linux native bindings now load on glibc 2.17 and later. They previously failed to load on systems older than glibc 2.35, such as Debian 11 and Red Hat Enterprise Linux 9.
