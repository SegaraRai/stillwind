---
"@stillwind/integration": patch
---

Astro sites that use the ClientRouter now keep Stillwind styles from client scripts after a client-side navigation. Previously, the stylesheet that a client script loaded was removed from the page head when the router swapped pages, so elements that the script created stayed unstyled until a full reload.
