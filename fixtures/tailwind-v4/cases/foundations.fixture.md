# Foundations

## `foundations.preflight-and-default-theme`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
block bg-red-500 font-sans
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', 'Noto Sans', Arial,
    sans-serif, 'Apple Color Emoji', 'Segoe UI Emoji', 'Segoe UI Symbol', 'Noto Color Emoji';
    --color-red-500: oklch(63.7% 0.237 25.331);
  }
}
@layer utilities {
  .block {
    display: block;
  }
  .bg-red-500 {
    background-color: var(--color-red-500);
  }
  .font-sans {
    font-family: var(--font-sans);
  }
}
```

## `foundations.theme-overrides-and-static-variables`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
@theme static {
  --color-brand: oklch(0.62 0.2 255);
  --spacing: 0.25rem;
}
```

### Candidates

```text
bg-brand p-4
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --spacing: 0.25rem;
    --color-brand: oklch(0.62 0.2 255);
  }
}
@layer utilities {
  .bg-brand {
    background-color: var(--color-brand);
  }
  .p-4 {
    padding: calc(var(--spacing) * 4);
  }
}
```
