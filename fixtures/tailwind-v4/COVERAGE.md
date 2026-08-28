# Tailwind CSS v4.3.3 coverage map

The source of truth for this suite is Tailwind CSS `v4.3.3`
(`c2b24dd15fed1c59dd521bd86082f520c9f5ad0d`). The fixture families mirror
the upstream compiler's public candidate and directive surface, rather than
Stillwind's current implementation details.

| Upstream surface | Fixture family | Representative coverage |
| --- | --- | --- |
| Theme, Preflight, CSS layers | `foundations` | default theme, reset, `@theme`, static variables |
| Accessibility and interaction | `accessibility-interaction` | a11y, pointer, touch, scroll, form controls |
| Layout and sizing | `layout`, `layout-positioning` | flex/grid, spacing, display, box model, position, overflow |
| Typography | `typography` | font, text, lists, truncation, text decoration |
| Backgrounds, borders, effects | `backgrounds-gradients`, `borders-effects` | colors, gradients, masks, borders, rings, shadows |
| Filters, transforms, motion | `filters-transforms`, `transitions-animation` | transforms, filters, transitions, animation |
| Responsive and container queries | `responsive-container` | breakpoints, range queries, containers |
| Structural and relational variants | `relational-variants`, `pseudo-elements` | group, peer, `has`, `in`, structural pseudo classes, pseudo elements |
| Media and preference variants | `media-preference-variants` | dark, print, reduced motion, orientation, contrast |
| Dynamic variant grammar | `variant-grammar` | arbitrary, compound, group/peer modifiers, aria/data, nth, supports, responsive, container, composition |
| Default variant surface | `catalog/variants` | the complete upstream `getVariants` IntelliSense metadata and parameter suggestions |
| Candidate grammar | `candidate-syntax` | negative, important, arbitrary property/value, `supports` |
| CSS directives and extension points | `custom-css` | `@utility`, `@custom-variant`, custom properties |
| CSS directives and source syntax | `directives`, `legacy-directives`, `resources` | `@apply`, `@variant`, `@tailwind utilities`, selector/block `@custom-variant`, inline `@source`, brace expansion, source negation, `@reference`, `@config`, `@plugin` |
| Direct upstream registrations | `builtins`, `catalog/direct-registrations` | every direct `staticUtility`, `functionalUtility`, static, functional, and compound variant registration |
| Default theme class space | `default-completions`, `catalog/default-completions` | every class in upstream's default `getClassList` IntelliSense snapshot |

Each row may contain several compact candidate sets. When adding a compiler
feature, put it in its semantic family (or add one family) and regenerate the
whole Markdown block with the pinned oracle. This keeps the suite small,
complete at the public-feature level, and mechanically reproducible.

The upstream test suites consulted when expanding this map are:

- `packages/tailwindcss/src/utilities.test.ts`
- `packages/tailwindcss/src/variants.test.ts`
- `packages/tailwindcss/src/candidate.test.ts`
- `packages/tailwindcss/src/index.test.ts`
