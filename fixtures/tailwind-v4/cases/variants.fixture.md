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

## `variants.arbitrary-selectors-without-nesting`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
[:hover]:flex [.is-open]:flex [[data-x]]:flex [:hover,:focus]:flex
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  .\[\.is-open\]\:flex:is(.is-open) {
    display: flex;
  }
  .\[\:hover\]\:flex:is(:hover) {
    display: flex;
  }
  .\[\:hover\,\:focus\]\:flex:is(:hover, :focus) {
    display: flex;
  }
  .\[\[data-x\]\]\:flex:is([data-x]) {
    display: flex;
  }
}
```

An arbitrary selector without `&` tests the styled element through `&:is(...)`.
Stillwind keeps a selector that `:is()` cannot contain or that never selects the
styled element, such as `[::selection]` or `[:root]`, as a document-wide rule and
reports `document-scoped-selector`, so this case does not compare those selectors.
