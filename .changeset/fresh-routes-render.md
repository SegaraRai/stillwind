---
"@stillwind/integration": patch
---

During Vite development, server rendering now shows a component edit even when a framework transforms the edited file for the server before the hot update runs. Previously, TanStack Start could keep rendering the component from before the edit until the server restarted.
