# Pseudo elements, generated content, and control affordances

## `variants.pseudo-elements-and-content`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
before:content-[''] before:block after:content-['→'] placeholder:text-red-500 file:bg-red-500 selection:bg-red-500 marker:text-red-500 first-letter:text-3xl
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
    --text-3xl: 1.875rem;
    --text-3xl--line-height: calc(2.25 / 1.875);
  }
}
@layer utilities {
  .first-letter\:text-3xl::first-letter {
    font-size: var(--text-3xl);
    line-height: var(--tw-leading, var(--text-3xl--line-height));
  }
  .marker\:text-red-500 ::marker {
    color: var(--color-red-500);
  }
  .marker\:text-red-500::marker {
    color: var(--color-red-500);
  }
  .marker\:text-red-500 ::-webkit-details-marker {
    color: var(--color-red-500);
  }
  .marker\:text-red-500::-webkit-details-marker {
    color: var(--color-red-500);
  }
  .selection\:bg-red-500 ::selection {
    background-color: var(--color-red-500);
  }
  .selection\:bg-red-500::selection {
    background-color: var(--color-red-500);
  }
  .file\:bg-red-500::file-selector-button {
    background-color: var(--color-red-500);
  }
  .placeholder\:text-red-500::placeholder {
    color: var(--color-red-500);
  }
  .before\:block::before {
    content: var(--tw-content);
    display: block;
  }
  .before\:content-\[\'\'\]::before {
    --tw-content: '';
    content: var(--tw-content);
  }
  .after\:content-\[\'→\'\]::after {
    --tw-content: '→';
    content: var(--tw-content);
  }
}
@property --tw-content {
  syntax: "*";
  initial-value: "";
  inherits: false;
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-content: "";
    }
  }
}
```
