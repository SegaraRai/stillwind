# Canonical candidates

Most cases come from Tailwind CSS 4.3.3's
[canonicalization tests](https://github.com/tailwindlabs/tailwindcss/blob/v4.3.3/packages/tailwindcss/src/canonicalize-candidates.test.ts).
Each file keeps the input CSS, the test's base theme, the options, and the expected candidates.
The tests in `tools/tailwind` check the expectations against the pinned Tailwind package.
The compiler integration test checks the same cases and verifies idempotence.
`39.json` also checks CSS order and explicit variable references against the pinned package.
`40.json` checks that named variants keep their spelling when variants are stacked.
`41.json` checks a larger group whose utilities have no shorter equivalent group.
`42.json` checks that comparisons keep custom property names and their case.

`stillwindExpected` records a specific supported difference without changing Tailwind's
expectation. Stillwind also matches equivalent color spellings to theme tokens when a
candidate has an opacity modifier. It therefore writes `bg-red-500/16` for
`bg-[#f00]/[0.16]`, where Tailwind keeps `bg-[#f00]/16`.
Stillwind also keeps explicit variable references when combining utilities, so
runtime changes to those variables still affect the result.

Tailwind's class prefix is not part of Stillwind's candidate syntax. Stillwind's
`prefix` and `tailwindPrefix` settings rename internal custom properties.
