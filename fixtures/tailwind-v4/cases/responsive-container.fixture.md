# Responsive media queries and container queries

## `responsive.breakpoints-and-containers`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
sm:grid md:grid-cols-3 lg:gap-8 max-md:hidden min-[900px]:flex @container @sm:grid @max-md:hidden
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --spacing: 0.25rem;
  }
}
@layer utilities {
  .\@container {
    container-type: inline-size;
  }
  @media (width < 48rem) {
    .max-md\:hidden {
      display: none;
    }
  }
  @media (width >= 900px) {
    .min-\[900px\]\:flex {
      display: flex;
    }
  }
  @media (width >= 40rem) {
    .sm\:grid {
      display: grid;
    }
  }
  @media (width >= 48rem) {
    .md\:grid-cols-3 {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }
  @media (width >= 64rem) {
    .lg\:gap-8 {
      gap: calc(var(--spacing) * 8);
    }
  }
  @container (width < 28rem) {
    .\@max-md\:hidden {
      display: none;
    }
  }
  @container (width >= 24rem) {
    .\@sm\:grid {
      display: grid;
    }
  }
}
```
