# Theme namespaces, replacement, variables, and negative resolution

## `theme.namespace-reset-and-negative-token`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
@theme {
  --color-*: initial;
  --color-brand: oklch(62% 0.2 280);
  --spacing: 0.5rem;
}
```

### Candidates

```text
bg-brand p-4 bg-red-500
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --spacing: 0.5rem;
    --color-brand: oklch(62% 0.2 280);
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

## `theme.inline-static-and-variable-reference`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
@theme inline {
  --color-action: var(--brand-color);
}
@theme static {
  --animate-enter: enter 200ms ease-out;
  @keyframes enter {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
}
```

### Candidates

```text
bg-action animate-enter [--brand-color:rebeccapurple]
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --animate-enter: enter 200ms ease-out;
  }
}
@layer utilities {
  .animate-enter {
    animation: var(--animate-enter);
  }
  .bg-action {
    background-color: var(--brand-color);
  }
  .\[--brand-color\:rebeccapurple\] {
    --brand-color: rebeccapurple;
  }
}
@keyframes enter {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}
```
