# Media, preference, and environment variants

## `variants.media-and-preferences`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
dark:bg-slate-950 motion-reduce:transition-none motion-safe:animate-spin print:hidden portrait:block landscape:flex forced-colors:appearance-auto contrast-more:border
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --color-slate-950: oklch(12.9% 0.042 264.695);
    --animate-spin: spin 1s linear infinite;
  }
}
@layer utilities {
  @media (prefers-reduced-motion: no-preference) {
    .motion-safe\:animate-spin {
      animation: var(--animate-spin);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .motion-reduce\:transition-none {
      transition-property: none;
    }
  }
  @media (prefers-contrast: more) {
    .contrast-more\:border {
      border-style: var(--tw-border-style);
      border-width: 1px;
    }
  }
  @media (orientation: portrait) {
    .portrait\:block {
      display: block;
    }
  }
  @media (orientation: landscape) {
    .landscape\:flex {
      display: flex;
    }
  }
  @media (prefers-color-scheme: dark) {
    .dark\:bg-slate-950 {
      background-color: var(--color-slate-950);
    }
  }
  @media print {
    .print\:hidden {
      display: none;
    }
  }
  @media (forced-colors: active) {
    .forced-colors\:appearance-auto {
      appearance: auto;
    }
  }
}
@property --tw-border-style {
  syntax: "*";
  inherits: false;
  initial-value: solid;
}
@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-border-style: solid;
    }
  }
}
```
