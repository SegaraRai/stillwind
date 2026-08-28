# Unicode identifiers, arbitrary values, and escaped output

## `unicode.identifiers-and-content`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
@theme {
  --font-東京: "Noto Sans JP", sans-serif;
  --breakpoint-東京: 60rem;
}
```

### Candidates

```text
font-東京 東京:grid content-['雪'] [--名前:緑] w-[calc(100%_-_var(--余白))]
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer utilities {
  .w-\[calc\(100\%_-_var\(--余白\)\)\] {
    width: calc(100% - var(--余白));
  }
  .content-\[\'雪\'\] {
    --tw-content: '雪';
    content: var(--tw-content);
  }
  .\[--名前\:緑\] {
    --名前: 緑;
  }
  @media (width >= 60rem) {
    .東京\:grid {
      display: grid;
    }
  }
}
@property --tw-content {
  syntax: "*";
  inherits: false;
  initial-value: "";
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-content: "";
    }
  }
}
```
