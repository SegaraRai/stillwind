# Review fixtures

This directory is the durable, review-facing record of Stillwind transforms.
Every fixture is a **project**, even when it contains one source module. That
keeps single-module and cross-module cases on the same model and lets a case
grow without another migration.

```text
fixtures/projects/<project-id>/
  project.json              # stable identity and ordered source list
  stillwind.css             # project theme
  input/<module path>       # authored source
  expected/<module path>    # compiler output, generated and reviewed
  expected/<module>.css     # module utility fragment; shared layers emit once
  expected/project.css      # complete formatted project stylesheet
```

`project.json` is schema-versioned. Project IDs and source paths are URL keys:
rename them only when intentionally breaking existing viewer permalinks.
Optional `compilerOptions` are passed to both module compilation and project CSS
rendering. The runtime-if projects compare fallback and native output. The color-fallback
projects compare output without a target, with modern targets, and with Safari 16.2.

Generate snapshots and compare them with:

```powershell
vp run generate:fixtures:projects
vp run test:fixtures:projects
vp run --filter @stillwind-internal/apps-fixtures-viewer build
```

`generate:fixtures:projects` compiles the whole source graph and uses the same TypeScript
fact propagation as the Vite integration. `test:fixtures:projects` fails when checked-in
expected output or the viewer's static catalog is stale.
Both tasks build the host and native dependencies through the task graph.
Generation updates the expected files and viewer catalog together; comparison
leaves those files unchanged. The viewer's `dev` and `build` tasks depend on generation.

Keep each project centered on one review narrative. Prefer a small matrix of
related transformations over one catch-all project, and use multiple source
files whenever import, re-export, alias, or conditional identity propagation is
part of the behavior under review.

The older `fixtures/tailwind-v4` Markdown suite remains authoritative for CSS
compatibility coverage, and package-local `tests/fixtures` remain test harness
inputs. Do not duplicate all of them here. Promote a small, representative case
to this directory when reviewers benefit from seeing its before/after output.
