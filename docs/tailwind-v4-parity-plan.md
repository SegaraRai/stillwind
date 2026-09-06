# Tailwind CSS v4 parity plan

## Goal

For the supported CSS-first surface (excluding `@source`, `@config`, `@plugin`,
and `@reference`), Stillwind must be checked against the pinned Tailwind CSS
oracle (`tailwindcss@4.3.3`). A candidate is compatible only when the minified
CSS is identical after replacing the generated utility class selector with a
canonical candidate selector. Acceptance must cover both utility declarations
and variant/at-rule structure; resolving a candidate is not sufficient.

## Non-goals

- Tailwind plugin execution.
- Tailwind content/source discovery and `className` scanning.
- The deferred directives `@source`, `@config`, `@plugin`, and `@reference`.

## Work phases

### 1. Oracle and parity harness

- Pin and record the Tailwind version used by the tools package.
- Compile the same `(input CSS, candidate set)` with Tailwind and Stillwind.
- Minify both outputs with the same Lightning CSS version.
- Canonicalize only generated class selectors; do not normalize declarations,
  values, at-rule conditions, layer names, selector ordering, or whitespace
  beyond the minifier's deterministic output.
- Fail with a focused diff containing the candidate, input CSS, oracle CSS, and
  Stillwind CSS.
- Replace the current unresolved-count-only 98% test with parity tests. Keep an
  acceptance report for unresolved candidates separately.

### 2. Candidate grammar parity

- Generate a grammar corpus from Tailwind's candidate parser tests and the
  pinned direct-registration/catalog fixtures.
- Add positive and negative cases for escaping, arbitrary values/properties,
  negative and important modifiers, fractions, slash ambiguity, nested
  brackets/functions, unicode escapes, and malformed candidates.
- Compare both acceptance and emitted CSS. Unknown/malformed candidates remain
  a Stillwind lint finding and must not be silently changed.
- Add differential tests for compound, arbitrary, named group/peer, container,
  responsive, media, `aria`, `data`, `not`, `in`, `has`, nth, and supports
  variants, including ordering and nesting combinations.

### 3. `@theme` semantic parity

- Build a namespace matrix from Tailwind's theme tests, including custom tokens,
  fallback behavior, `--default-*`, static/inline behavior, namespace reset,
  inferred tokens, and declaration order.
- For every matrix row, compare minified generated CSS and the set of emitted
  `@property` registrations.
- Preserve authored `@property` rules and verify Stillwind-owned
  `@stillwind-property` output is emitted only when referenced.
- Add regression tests for multiple theme blocks, imports, empty namespaces,
  and user CSS interleaved with theme/builtin imports.

### 4. Utility and variant output parity

- Use the generated direct-registration catalog as the inventory, but require
  output parity rather than registration-name parity.
- Compare all static, functional, and compound registrations with representative
  values from Tailwind's default completion catalog.
- Compare layer placement, selector lists, declaration order, value spelling,
  vendor-prefixed declarations, media/supports conditions, and variant order.
- Add a small set of full CSS snapshot tests for preflight, theme, utilities,
  and builtins imports to catch composition regressions.

### 5. CI gates and maintenance

- `fixtures:check` verifies oracle fixtures are current.
- A dedicated `tailwind:parity` task runs the minified differential suite.
- CI fails on any parity mismatch; unresolved candidates are reported but are
  not counted as parity successes.
- Regeneration is explicit and records the Tailwind version and Lightning CSS
  version in the generated catalog header.

## Definition of done

1. Every candidate in the supported grammar corpus has either an exact CSS
   parity test or an explicit unsupported classification.
2. All supported `@theme` behaviors have oracle comparisons.
3. Utility and variant snapshots are compared after minification with generated
   class names excluded only at selector positions.
4. The old 98% acceptance test no longer serves as the compatibility gate.
5. Any remaining mismatch is surfaced with a reproducible fixture and a
   candidate-level diff.

## Current implementation status (2026-09-07)

- The Rust representative gate now compares minified declarations and variant
  structure; the former unresolved-count acceptance test is removed.
- `tools/tailwind-parity.mts` provides the full candidate-level differential
  oracle and is available as `vp run tailwind:parity:all`. It uses the pinned
  Tailwind compiler and Lightning CSS minifier and reports every mismatch.
- Candidate grammar coverage includes arbitrary values/properties, important,
  negative, fractions, responsive/container, `aria`, `data`, `supports`, dark,
  and selector variants.
- `@theme inline`/`static` handling, referenced-token tree shaking, and
  Stillwind-owned property registration are covered by focused tests.
- The current full-catalog baseline is 13,145/13,229 exact matches (two cases
  are excluded because they use out-of-scope directives). Recent fixes cover
  authored property precedence, inset-ring and scrollbar fallbacks, mask
  current-color handling, conic angle spelling and axis ordering, backdrop
  property names, decoration/ring fallbacks, and drop-shadow filter-family
  ordering, plus linear-gradient angle fallbacks.
  Remaining failures are intentionally visible and are concentrated in unsupported registrations plus transform
  spelling, border/divide selector/property registration, shadow token values,
  and several advanced variant cases. This is an active parity backlog, not an
  acceptance claim.
- The focused 50-case gate is currently 50/50. Shadow opacity now emits
  Tailwind's two-stage hex fallback and nested relative-color emission.
