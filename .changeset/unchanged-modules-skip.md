---
"@stillwind/integration": patch
---

Compile faster when many modules import the same styles. A compilation no longer rebuilds the dependencies whose source did not change, and development servers no longer render the whole stylesheet again when a compilation leaves its producers unchanged. Editing a shared style module in a Vite development server now applies the change sooner.
