# CSS-first directives and source syntax

## `directives.apply-and-variant`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
@layer components {
  .button {
    @apply inline-flex bg-red-500 px-4;
    @variant hover {
      opacity: 0.8;
    }
  }
}
```

### Candidates

```text

```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
    --spacing: 0.25rem;
  }
}
@layer utilities;
@layer components {
  .button {
    display: inline-flex;
    background-color: var(--color-red-500);
    padding-inline: calc(var(--spacing) * 4);
    &:hover {
      @media (hover: hover) {
        opacity: 0.8;
      }
    }
  }
}
```

## `directives.apply-order-sensitive-spacing`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
@layer components {
  .spacing {
    @apply m-4 ml-2 p-4 px-2;
  }
}
```

### Candidates

```text

```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --spacing: 0.25rem;
  }
}
@layer utilities;
@layer components {
  .spacing {
    margin: calc(var(--spacing) * 4);
    margin-left: calc(var(--spacing) * 2);
    padding: calc(var(--spacing) * 4);
    padding-inline: calc(var(--spacing) * 2);
  }
}
```

## `directives.apply-order-sensitive-radius`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
@layer components {
  .radius {
    @apply rounded-lg rounded-t-sm;
  }
}
```

### Candidates

```text

```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --radius-sm: 0.25rem;
    --radius-lg: 0.5rem;
  }
}
@layer utilities;
@layer components {
  .radius {
    border-radius: var(--radius-lg);
    border-top-left-radius: var(--radius-sm);
    border-top-right-radius: var(--radius-sm);
  }
}
```

## `directives.apply-order-sensitive-same-property`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
@layer components {
  .opacity {
    @apply opacity-50 opacity-75;
  }
}
```

### Candidates

```text

```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities;
@layer components {
  .opacity {
    opacity: 50%;
    opacity: 75%;
  }
}
```

## `directives.inline-source-brace-expansion-and-negation`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
@source inline("{hover:,focus:,}underline {,sm:}flex");
@source not inline("focus:underline");
```

### Candidates

```text

```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  .flex {
    display: flex;
  }
  .underline {
    text-decoration-line: underline;
  }
  @media (hover: hover) {
    .hover\:underline:hover {
      text-decoration-line: underline;
    }
  }
  @media (width >= 40rem) {
    .sm\:flex {
      display: flex;
    }
  }
}
```

## `directives.reference-apply`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
@reference "./resources/reference.css";
@layer components {
  .referenced {
    @apply text-reference;
  }
}
```

### Candidates

```text

```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities;
@layer components {
  .referenced {
    color: var(--color-reference, rebeccapurple);
  }
}
```

## `directives.legacy-config-and-plugin`

### Input CSS

```css
@config "./resources/tailwind.config.cjs";
@plugin "./resources/plugin.cjs";
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
bg-legacy legacy-skew
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  .legacy-skew {
    transform: skewY(-10deg);
  }
  .bg-legacy {
    background-color: rebeccapurple;
  }
}
```
