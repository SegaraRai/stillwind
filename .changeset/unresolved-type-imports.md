---
"@stillwind/integration": patch
---

The Vite integration no longer stops a build when an import that Stillwind reads for type facts does not resolve. For example, a Svelte component with `import type { HTMLAttributes } from "svelte/elements"` now builds without an alias, although `svelte/elements` exports that path only under the `types` condition.
