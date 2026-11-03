---
"@stillwind/vscode-extension": patch
---

The VS Code extension now starts when you open a Vue, Svelte, or Astro file, even if the workspace root has no `stillwind.config.*`. Previously, a workspace with its configuration only in a subfolder got no diagnostics or completion in these files until a JavaScript, TypeScript, or CSS file was opened. The Marketplace and Open VSX listings now show keywords, a banner, and a description that names Tailwind CSS v4.
