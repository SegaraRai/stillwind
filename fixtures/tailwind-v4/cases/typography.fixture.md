# Typography, text flow, and lists

## `typography.core-and-text-flow`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
font-serif text-3xl text-red-500 text-center font-bold italic underline decoration-2 underline-offset-4 leading-tight tracking-wide line-clamp-3 truncate whitespace-nowrap break-words hyphens-auto list-disc list-inside
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --font-serif: ui-serif, Georgia, Cambria, 'Times New Roman', Times, serif;
    --color-red-500: oklch(63.7% 0.237 25.331);
    --text-3xl: 1.875rem;
    --text-3xl--line-height: calc(2.25 / 1.875);
    --font-weight-bold: 700;
    --tracking-wide: 0.025em;
    --leading-tight: 1.25;
  }
}
@layer utilities {
  .line-clamp-3 {
    overflow: hidden;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
  }
  .list-inside {
    list-style-position: inside;
  }
  .list-disc {
    list-style-type: disc;
  }
  .truncate {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .text-center {
    text-align: center;
  }
  .font-serif {
    font-family: var(--font-serif);
  }
  .text-3xl {
    font-size: var(--text-3xl);
    line-height: var(--tw-leading, var(--text-3xl--line-height));
  }
  .leading-tight {
    --tw-leading: var(--leading-tight);
    line-height: var(--leading-tight);
  }
  .font-bold {
    --tw-font-weight: var(--font-weight-bold);
    font-weight: var(--font-weight-bold);
  }
  .tracking-wide {
    --tw-tracking: var(--tracking-wide);
    letter-spacing: var(--tracking-wide);
  }
  .break-words {
    overflow-wrap: break-word;
  }
  .hyphens-auto {
    -webkit-hyphens: auto;
    hyphens: auto;
  }
  .whitespace-nowrap {
    white-space: nowrap;
  }
  .text-red-500 {
    color: var(--color-red-500);
  }
  .italic {
    font-style: italic;
  }
  .underline {
    text-decoration-line: underline;
  }
  .decoration-2 {
    text-decoration-thickness: 2px;
  }
  .underline-offset-4 {
    text-underline-offset: 4px;
  }
}
@property --tw-leading {
  syntax: "*";
  inherits: false;
}
@property --tw-font-weight {
  syntax: "*";
  inherits: false;
}
@property --tw-tracking {
  syntax: "*";
  inherits: false;
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-leading: initial;
      --tw-font-weight: initial;
      --tw-tracking: initial;
    }
  }
}
```
