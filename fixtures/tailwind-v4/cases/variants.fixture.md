# Variants and selectors

## `variants.state-attribute-responsive-and-arbitrary`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
hover:bg-red-500 focus-visible:outline-2 aria-[busy=true]:opacity-60 data-[state=open]:grid md:px-4 [&>svg]:size-4
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
    --spacing: 0.25rem;
  }
}
@layer utilities {
  @media (hover: hover) {
    .hover\:bg-red-500:hover {
      background-color: var(--color-red-500);
    }
  }
  .focus-visible\:outline-2:focus-visible {
    outline-style: var(--tw-outline-style);
    outline-width: 2px;
  }
  .aria-\[busy\=true\]\:opacity-60[aria-busy="true"] {
    opacity: 60%;
  }
  .data-\[state\=open\]\:grid[data-state="open"] {
    display: grid;
  }
  @media (width >= 48rem) {
    .md\:px-4 {
      padding-inline: calc(var(--spacing) * 4);
    }
  }
  .\[\&\>svg\]\:size-4 > svg {
    width: calc(var(--spacing) * 4);
    height: calc(var(--spacing) * 4);
  }
}
@property --tw-outline-style {
  syntax: "*";
  inherits: false;
  initial-value: solid;
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-outline-style: solid;
    }
  }
}
```
