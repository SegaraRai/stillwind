# Tailwind CSS v4 compatibility fixtures

These fixtures use the pinned Tailwind compiler as the oracle. Stillwind does
not generate their expected CSS.

## Case format

`cases/*.fixture.md` groups cases by capability. Each case contains Input CSS,
Candidates, and Expected CSS fenced blocks. `tools/tailwind/fixtures.ts` passes
the input and explicit candidates to the Tailwind compiler API. Its stylesheet
loader resolves the installed Tailwind stylesheets and fixture-relative imports.
Its module loader resolves fixture-relative configuration and plugin modules.
The compiler output is preserved apart from line-ending normalization and
trailing whitespace removal.

Ordinary cases import `tailwindcss/theme` and `tailwindcss/utilities`. Reset cases
use the complete `tailwindcss` import and include Preflight. The generator
enforces a maximum of 500 fixture files.

```powershell
vp run generate:fixtures:tailwind
vp run @stillwind-internal/tools-tailwind#check:generated:fixtures:tailwind
```

The check compares the Expected CSS blocks without updating them.

Generator scripts write files by default; `--check` compares without writing.
`tools/tailwind/utility-order.ts` generates the Rust property-order table.
`tools/tailwind/utility-order-fixtures.ts` generates `utility-order.json`, which
the Rust differential ordering test reads. Update that fixture with:

```powershell
vp run @stillwind-internal/tools-tailwind#generate:fixtures:tailwind-order
```

## Upstream registration catalog

`tools/tailwind/upstream-source.json` pins the Tailwind version, full Git commit
SHA, and SHA-256 hashes of the four source files used by registration generation.
`prepare:tailwind-source` downloads missing files from that commit into
`tools/tailwind/.cache/upstream/<SHA>` and deletes that directory if acquisition
or verification of newly downloaded files fails. It verifies an existing cache
without downloading or writing.
Vite+ tracks cache reads and writes automatically, so missing cached files invalidate
the task result when preparation only verified an existing directory.

Preparation reads the installed `tailwindcss/package.json`. If `upstream-source.json`
is missing or its version differs, preparation resolves that release's Git tag and
creates the JSON with its commit SHA and file hashes. `upstream.ts` defines the
four files to download, so acquisition does not require an existing source JSON.

`tools/tailwind/registrations.ts` verifies the pinned hashes and installed Tailwind
version before reading registration sources or writing catalogs and candidate lists.
Both registration tasks depend on `prepare:tailwind-source`, so a fresh clone only
needs `vp i` and network access when the source files are not cached.

The fixture task generates candidate lists before their Expected CSS:

```powershell
vp run generate:fixtures:tailwind
```

The direct `@stillwind-internal/tools-tailwind#generate:tailwind-registrations` task
updates catalogs and candidate lists. Its `check:tailwind-registrations`
counterpart compares them without writing. Run
`generate:fixtures:tailwind` after it to update Expected CSS. These operations
write the same Markdown files and must run in sequence.

## Oracle and comparison packages

`@stillwind-internal/tools-tailwind` owns the pinned upstream compiler, fixture generation,
and ordering data. It has no dependency on the Stillwind compiler or host.
`@stillwind-internal/tools-tailwind-compat` uses the native binding to compare Stillwind
output with those fixtures and the upstream compiler. Run `vp run test:tailwind`
for the native comparisons and `vp run test:browser` for browser contracts.

A Tailwind upgrade changes the workspace catalog entry and runs `vp i`.
Preparation updates `upstream-source.json` to the version in the installed
`tailwindcss/package.json`. Run `vp run generate` to update fixtures and ordering
data, and review the generated files together. `check:generated` compares the
Rust property table and ordering fixture through separate tasks.
