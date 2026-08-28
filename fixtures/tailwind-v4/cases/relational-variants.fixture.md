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
