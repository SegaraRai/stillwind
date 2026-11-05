---
"@stillwind/integration": patch
---

A SvelteKit server that uses `@stillwind/integration/sveltekit/handle` now starts when the package is installed from npm. Previously, the SSR build left the handle as an external import, so a production server, for example one built with `@sveltejs/adapter-node`, failed to start with `Cannot find package '$app'`. You can remove an `ssr.noExternal` entry that you added for `@stillwind/integration` as a workaround.
