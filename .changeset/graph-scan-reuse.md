---
"@stillwind/integration": patch
---

Update document CSS faster after an edit in Next and Turbopack development. Document CSS reconstruction now reuses the imports of modules whose source is unchanged and reads each level of the module graph concurrently, and Next route discovery reads route directories concurrently. Adding or removing a file still makes reconstruction resolve every import again.
