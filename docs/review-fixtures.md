# Review fixtures and viewer

Review fixtures make compiler behavior reviewable as concrete before/after
artifacts. They complement unit and compatibility tests; they are not a second
copy of every test input.

## Structure and migration

Every case under `fixtures/projects` is a project. A project has a stable
`project.json`, one theme, an ordered source graph under `input`, and generated
snapshots under `expected`. A single-module transform still uses this structure
with a one-item `sources` array.

Existing fixtures stay where their test harnesses expect them:

- `fixtures/tailwind-v4` remains the broad Markdown-driven CSS compatibility
  suite.
- package and crate `test/fixtures` remain local inputs for their tests.
- `fixtures/projects` contains selected review narratives, including cases that
  span modules.

Promote or adapt a representative test rather than moving it. This preserves
test paths and avoids making the viewer's curated catalog as noisy as the full
test corpus. A later migration can add a manifest reference to an existing test
fixture if duplication becomes material.

## Generate and serve

```powershell
vp run fixtures:refresh
vp run fixtures:check
vp run --filter @stillwind/fixtures-viewer dev
vp run --filter @stillwind/fixtures-viewer build
```

The refresh task compiles each complete project, collects TypeScript facts, and
writes runnable transformed source and per-module utility CSS fragments. Shared
theme/base/property layers are deliberately omitted from each fragment because
the Vite integration emits them once for the complete module graph. It also creates the static JSON
catalog consumed by the viewer. The check task regenerates in memory and fails
when a manifest, source, expected file, or catalog is stale. The production
viewer is static and read-only; its host needs no compiler or API route.

## URLs and search

Viewer links use a hash route so they survive deployment on any static host and
under any base path:

```text
https://<viewer-host>/#/projects/<project-id>/files/<percent-encoded-source-path>
```

For example:

```text
/#/projects/view-transition-matrix/files/src%2Ftransitions.ts
```

Project IDs and source paths are therefore persistent public identifiers. The
viewer searches project titles, IDs, tags, source paths, declared symbols
(`class`, `function`, `interface`, `type`, and variables), and source text in a
small in-browser index. No search service or runtime write access is required.

## Review rule

For any feature or behavior change that alters emitted source or CSS:

1. Add or update the smallest representative project.
2. Run `vp run fixtures:refresh` and review both transformed source and CSS.
3. Run `vp run fixtures:check` with the normal affected tests.
4. Put the relevant viewer permalink in the PR or issue description.
5. Treat an expected-output diff as a behavior change; explain it rather than
   accepting snapshots mechanically.

Do not rename an existing project ID or source path casually: that invalidates
links already used in reviews. Add a new project when a fundamentally different
narrative needs a stable URL.
