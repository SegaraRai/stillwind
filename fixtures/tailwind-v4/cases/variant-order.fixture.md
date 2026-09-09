# Variant generation order

These cases deliberately list candidates out of canonical order. Every
candidate uses the same utility so the snapshots isolate Tailwind's variant
ordering from utility declaration ordering.

## `variant-order.states`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
disabled:block active:block focus-visible:block hover:block checked:block focus:block focus-within:block visited:block invalid:block open:block
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  .visited\:block:visited {
    display: block;
  }
  .open\:block:is([open], :popover-open, :open) {
    display: block;
  }
  .checked\:block:checked {
    display: block;
  }
  .invalid\:block:invalid {
    display: block;
  }
  .focus-within\:block:focus-within {
    display: block;
  }
  @media (hover: hover) {
    .hover\:block:hover {
      display: block;
    }
  }
  .focus\:block:focus {
    display: block;
  }
  .focus-visible\:block:focus-visible {
    display: block;
  }
  .active\:block:active {
    display: block;
  }
  .disabled\:block:disabled {
    display: block;
  }
}
```

## `variant-order.conditions`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
print:block dark:block forced-colors:block landscape:block portrait:block contrast-more:block motion-reduce:block motion-safe:block max-md:block max-lg:block min-[900px]:block min-md:block sm:block md:block supports-[display:grid]:block @max-md:block @max-lg:block @sm:block @md:block @min-md:block
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  @supports (display:grid) {
    .supports-\[display\:grid\]\:block {
      display: block;
    }
  }
  @media (prefers-reduced-motion: no-preference) {
    .motion-safe\:block {
      display: block;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .motion-reduce\:block {
      display: block;
    }
  }
  @media (prefers-contrast: more) {
    .contrast-more\:block {
      display: block;
    }
  }
  @media (width < 64rem) {
    .max-lg\:block {
      display: block;
    }
  }
  @media (width < 48rem) {
    .max-md\:block {
      display: block;
    }
  }
  @media (width >= 900px) {
    .min-\[900px\]\:block {
      display: block;
    }
  }
  @media (width >= 40rem) {
    .sm\:block {
      display: block;
    }
  }
  @media (width >= 48rem) {
    .md\:block {
      display: block;
    }
    .min-md\:block {
      display: block;
    }
  }
  @container (width < 32rem) {
    .\@max-lg\:block {
      display: block;
    }
  }
  @container (width < 28rem) {
    .\@max-md\:block {
      display: block;
    }
  }
  @container (width >= 24rem) {
    .\@sm\:block {
      display: block;
    }
  }
  @container (width >= 28rem) {
    .\@md\:block {
      display: block;
    }
    .\@min-md\:block {
      display: block;
    }
  }
  @media (orientation: portrait) {
    .portrait\:block {
      display: block;
    }
  }
  @media (orientation: landscape) {
    .landscape\:block {
      display: block;
    }
  }
  @media (prefers-color-scheme: dark) {
    .dark\:block {
      display: block;
    }
  }
  @media print {
    .print\:block {
      display: block;
    }
  }
  @media (forced-colors: active) {
    .forced-colors\:block {
      display: block;
    }
  }
}
```

## `variant-order.mixed`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
md:block peer-focus:block hover:block data-[state=open]:block first:block dark:block group-hover:block group-checked:block group-focus:block peer-checked:block peer-hover:block focus:block aria-[busy=true]:block supports-[display:grid]:block not-hover:block has-focus:block md:hover:block hover:md:block
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  .not-hover\:block:not(:hover) {
    display: block;
  }
  @media not (hover: hover) {
    .not-hover\:block {
      display: block;
    }
  }
  .group-checked\:block:is(:where(.group):checked *) {
    display: block;
  }
  @media (hover: hover) {
    .group-hover\:block:is(:where(.group):hover *) {
      display: block;
    }
  }
  .group-focus\:block:is(:where(.group):focus *) {
    display: block;
  }
  .peer-checked\:block:is(:where(.peer):checked ~ *) {
    display: block;
  }
  @media (hover: hover) {
    .peer-hover\:block:is(:where(.peer):hover ~ *) {
      display: block;
    }
  }
  .peer-focus\:block:is(:where(.peer):focus ~ *) {
    display: block;
  }
  .first\:block:first-child {
    display: block;
  }
  @media (hover: hover) {
    .hover\:block:hover {
      display: block;
    }
  }
  .focus\:block:focus {
    display: block;
  }
  .has-focus\:block:has(:focus) {
    display: block;
  }
  .aria-\[busy\=true\]\:block[aria-busy="true"] {
    display: block;
  }
  .data-\[state\=open\]\:block[data-state="open"] {
    display: block;
  }
  @supports (display:grid) {
    .supports-\[display\:grid\]\:block {
      display: block;
    }
  }
  @media (width >= 48rem) {
    .md\:block {
      display: block;
    }
  }
  @media (hover: hover) {
    @media (width >= 48rem) {
      .hover\:md\:block:hover {
        display: block;
      }
    }
  }
  @media (width >= 48rem) {
    @media (hover: hover) {
      .md\:hover\:block:hover {
        display: block;
      }
    }
  }
  @media (prefers-color-scheme: dark) {
    .dark\:block {
      display: block;
    }
  }
}
```
