# Structural, relational, and state variants

## `variants.relational-and-structural`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
group group-hover:text-red-500 group-focus-within:block peer peer-checked:bg-red-500 has-checked:bg-red-500 in-focus:opacity-100 not-hover:opacity-100 odd:bg-red-500 first:font-bold
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
    --font-weight-bold: 700;
  }
}
@layer utilities {
  .not-hover\:opacity-100:not(:hover) {
    opacity: 100%;
  }
  @media not (hover: hover) {
    .not-hover\:opacity-100 {
      opacity: 100%;
    }
  }
  .group-focus-within\:block:is(:where(.group):focus-within *) {
    display: block;
  }
  @media (hover: hover) {
    .group-hover\:text-red-500:is(:where(.group):hover *) {
      color: var(--color-red-500);
    }
  }
  .peer-checked\:bg-red-500:is(:where(.peer):checked ~ *) {
    background-color: var(--color-red-500);
  }
  .first\:font-bold:first-child {
    --tw-font-weight: var(--font-weight-bold);
    font-weight: var(--font-weight-bold);
  }
  .odd\:bg-red-500:nth-child(odd) {
    background-color: var(--color-red-500);
  }
  :where(:focus) .in-focus\:opacity-100 {
    opacity: 100%;
  }
  .has-checked\:bg-red-500:has(:checked) {
    background-color: var(--color-red-500);
  }
}
@property --tw-font-weight {
  syntax: "*";
  inherits: false;
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-font-weight: initial;
    }
  }
}
```

## `variants.negation`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
not-focus:flex not-hover:flex not-[:hover]:flex not-[.foo_&]:flex not-in-data-open:flex not-group-hover:flex not-group-hover/menu:flex not-group-has-checked/menu:flex has-group-hover/menu:flex in-group-hover/menu:flex not-sm:flex not-max-sm:flex not-min-[600px]:flex not-not-sm:flex not-print:flex not-dark:flex not-motion-reduce:flex not-[@media_not_print]:flex not-supports-[display:grid]:flex not-[@supports_not_(display:grid)]:flex not-@md:flex not-@md/side:flex
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  @media (width >= 40rem) {
    .not-not-sm\:flex {
      display: flex;
    }
  }
  .not-group-hover\:flex:not(:is(:where(.group):hover *)) {
    display: flex;
  }
  @media not (hover: hover) {
    .not-group-hover\:flex {
      display: flex;
    }
  }
  .not-group-hover\/menu\:flex:not(:is(:where(.group\/menu):hover *)) {
    display: flex;
  }
  @media not (hover: hover) {
    .not-group-hover\/menu\:flex {
      display: flex;
    }
  }
  .not-group-has-checked\/menu\:flex:not(:is(:where(.group\/menu):has(:checked) *)) {
    display: flex;
  }
  .not-hover\:flex:not(:hover) {
    display: flex;
  }
  @media not (hover: hover) {
    .not-hover\:flex {
      display: flex;
    }
  }
  .not-focus\:flex:not(:focus) {
    display: flex;
  }
  .not-in-data-open\:flex:not(:where([data-open]) *) {
    display: flex;
  }
  @supports not (display:grid) {
    .not-supports-\[display\:grid\]\:flex {
      display: flex;
    }
  }
  @media not (prefers-reduced-motion: reduce) {
    .not-motion-reduce\:flex {
      display: flex;
    }
  }
  @media not (width < 40rem) {
    .not-max-sm\:flex {
      display: flex;
    }
  }
  @media not (width >= 600px) {
    .not-min-\[600px\]\:flex {
      display: flex;
    }
  }
  @media not (width >= 40rem) {
    .not-sm\:flex {
      display: flex;
    }
  }
  @container side not (width >= 28rem) {
    .not-\@md\/side\:flex {
      display: flex;
    }
  }
  @container not (width >= 28rem) {
    .not-\@md\:flex {
      display: flex;
    }
  }
  @media not (prefers-color-scheme: dark) {
    .not-dark\:flex {
      display: flex;
    }
  }
  @media not print {
    .not-print\:flex {
      display: flex;
    }
  }
  .not-\[\:hover\]\:flex:not(:is(:hover)) {
    display: flex;
  }
  .not-\[\.foo_\&\]\:flex:not(.foo *) {
    display: flex;
  }
  @media print {
    .not-\[\@media_not_print\]\:flex {
      display: flex;
    }
  }
  @supports (display:grid) {
    .not-\[\@supports_not_\(display\:grid\)\]\:flex {
      display: flex;
    }
  }
  @media (hover: hover) {
    :where(:is(:where(.group\/menu):hover *)) .in-group-hover\/menu\:flex {
      display: flex;
    }
    .has-group-hover\/menu\:flex:has(:is(:where(.group\/menu):hover *)) {
      display: flex;
    }
  }
}
```

Stillwind also resolves negations that Tailwind rejects or emits as invalid CSS, so
these candidates are not compared with Tailwind:

- A negated alternative requires every negated branch. `not-*` of a custom variant
  with the rule blocks `&:hover` and `&:focus` emits `&:not(:hover):not(:focus)`.
- A negated negation restores the original conditions. `not-not-hover` emits
  `&:is(:hover)` inside `@media (hover: hover)`.
- A negated media query list requires every negated query, and a combined condition
  is parenthesized: `not-[@media_(a)_and_(b)]` emits `@media not ((a) and (b))`.

Stillwind rejects negations of selectors that style another element. Tailwind emits
`not-[&_.foo]` as `:not(* .foo)`, which tests the styled element instead of its
`.foo` descendants. `not-*` and `not-**` are rejected for the same reason.

Both compilers reject `not-before`, `not-placeholder`, `not-file`, and `not-starting`,
because their variants contain pseudo-elements, declarations, or `@starting-style`.
