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

## Upstream data

Acquisition, hash validation, task ordering, and generated-data ownership are
documented in [Updating generated files](../../docs/test-inventory.md#updating-generated-files).
Registration catalogs and expected CSS are updated sequentially by
`vp run generate:fixtures:tailwind`.

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
