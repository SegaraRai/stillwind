# CSS-first customisation

## `custom-css.utility-variant-and-custom-property`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
@utility content-auto {
  content-visibility: auto;
}
@custom-variant data-checked (&[data-ui~="checked"]);
```

### Candidates

```text
content-auto data-checked:content-auto [--gutter:1rem] w-[calc(var(--gutter)*2)]
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  .w-\[calc\(var\(--gutter\)\*2\)\] {
    width: calc(var(--gutter) * 2);
  }
  .\[--gutter\:1rem\] {
    --gutter: 1rem;
  }
  .content-auto {
    content-visibility: auto;
  }
  .data-checked\:content-auto[data-ui~="checked"] {
    content-visibility: auto;
  }
}
```

## `custom-css.theme-inline-functional-utility-and-multi-selector-variant`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
@theme inline {
  --color-brand: var(--brand);
}
@utility tab-* {
  tab-size: --value(integer);
}
@custom-variant hocus (&:hover, &:focus);
```

### Candidates

```text
bg-brand tab-8 hocus:bg-brand
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  .bg-brand {
    background-color: var(--brand);
  }
  .tab-8 {
    tab-size: 8;
  }
  .hocus\:bg-brand:hover, .hocus\:bg-brand:focus {
    background-color: var(--brand);
  }
}
```

## `custom-css.prefix-migration`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme) prefix(tw);
@import "tailwindcss/utilities" layer(utilities) prefix(tw);
@theme {
  --color-brand: rebeccapurple;
}
```

### Candidates

```text
tw:flex tw:bg-brand
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --tw-color-brand: rebeccapurple;
  }
}
@layer utilities {
  .tw\:flex {
    display: flex;
  }
  .tw\:bg-brand {
    background-color: var(--tw-color-brand);
  }
}
```
