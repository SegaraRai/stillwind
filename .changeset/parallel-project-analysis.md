---
"@stillwind/cli": minor
"@stillwind/analysis": minor
---

`stillwind lint`, `fmt`, and `check` now analyze the projects of a configuration in parallel worker processes, so a configuration that lists several projects, such as a monorepo root, finishes sooner. Projects that share a target file are still analyzed together in one process. `lint`, `format`, and `check` in `@stillwind/analysis` accept a `concurrency` option that sets the maximum number of worker processes. By default, the maximum is half of the available parallelism.
