---
"stillwind": minor
---

Export `StyleResultOf` from each framework entrypoint, such as `stillwind/react`. TypeScript can now emit declaration files for modules that export a `style()` result, for example with `declaration: true` or the `dts` option of `vp pack`. Before, this failed with TS2883 because the type of the result could not be named from a public entrypoint.
