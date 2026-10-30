---
title: Design systems
description: Define colors, spacing, radii, breakpoints, and text sizes once, and report values that bypass them.
slug: docs/guides/design-systems
---

You want every component to use the same colors, spacing, and breakpoints.
Tailwind accepts arbitrary values such as `bg-[#3b82f6]` and `min-[713px]:`,
which is useful for one-off designs. In a shared codebase, you may want every
value to come from your tokens instead. In Stillwind, you define tokens in your
stylesheet and can enable rules that report values outside them.

## Define tokens by role

Add tokens to the stylesheet that `stillwind.config.ts` selects. Name each token
after its role, such as `action` or `panel`, instead of its value:

```css
@import "stillwind";

@theme {
  --color-background: light-dark(#f4f7fc, #131b2b);
  --color-foreground: light-dark(#202b45, #e4edff);
  --color-surface: light-dark(#ffffff, #1b263b);
  --color-action: light-dark(#354fa2, #b6cdff);
  --color-action-foreground: light-dark(#f7faff, #172448);
  --spacing-panel: 1.5rem;
  --radius-control: 0.25rem;
  --radius-panel: 0.5rem;
  --breakpoint-sidebar: 52.5rem;
  --text-ui: 0.8125rem;
}

:root {
  color-scheme: light dark;
}
```

Each token adds utilities. For example, `--color-action` adds `bg-action` and
`text-action`, and `--spacing-panel` adds `p-panel` and `gap-panel`.

`light-dark()` selects the first value in a light color scheme and the second
value in a dark one. Components use the same utility names in both schemes.

## Use the tokens in components

```tsx
import { style } from "stillwind/react";

export function Panel() {
  return (
    <section {...style("rounded-panel bg-surface p-panel text-foreground sidebar:grid")}>
      <button {...style("rounded-control bg-action px-4 py-2 text-action-foreground text-ui")}>
        Continue
      </button>
    </section>
  );
}
```

When a theme changes, you edit the stylesheet. The components stay the same.

To use a breakpoint in JavaScript, pass its name to `style.media()`. See
[Responsive styles](/docs/guides/responsive-styles).

## Report values outside the theme

Enable these rules in `stillwind.config.ts`:

```ts title="stillwind.config.ts"
import { defineConfig, lintPresets } from "stillwind/config";

export default defineConfig({
  projects: [
    {
      css: "./src/stillwind.css",
      files: ["src/**/*.{ts,tsx,vue,svelte,astro}"],
      lint: {
        extends: [lintPresets.recommended],
        rules: {
          "no-arbitrary-colors": "deny",
          "no-arbitrary-spacing": "deny",
          "no-arbitrary-breakpoints": "deny",
        },
      },
    },
  ],
});
```

| Rule                                                             | Reports                           | Example        |
| ---------------------------------------------------------------- | --------------------------------- | -------------- |
| [no-arbitrary-colors](/docs/lints/no-arbitrary-colors)           | Colors written as values          | `bg-[#123456]` |
| [no-arbitrary-spacing](/docs/lints/no-arbitrary-spacing)         | Margin, padding, and gap values   | `p-[13px]`     |
| [no-arbitrary-breakpoints](/docs/lints/no-arbitrary-breakpoints) | Viewport widths written as values | `min-[713px]:` |

No preset enables these rules, so add them yourself. To enforce them, run the
CLI in CI, as described in [Checks in the editor and CI](/docs/guides/lint-adoption).

## Reserve the default palette

The default theme also contains palette colors such as `blue-500`. To make
components use your role names instead, report the palette tokens:

```ts
const rules = {
  "no-restricted-theme-tokens": {
    severity: "deny",
    tokens: ["--color-blue-*", "--color-gray-*"],
  },
};
```

The rule reports utilities that use a matching token, such as `bg-blue-500`.
Your role tokens can still use palette colors in their definitions.

For more restrictions, such as banned utilities or length units, see
[Configuration](/docs/configuration#rule-options).
