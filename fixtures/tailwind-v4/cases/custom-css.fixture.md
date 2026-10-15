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

## `custom-css.variants-without-nesting-selector`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
@custom-variant hv (:hover);
@custom-variant op (.open);
@custom-variant child (> .item);
@custom-variant either (:hover, .open);
@custom-variant blk {
  :hover {
    @slot;
  }
}
@custom-variant blk-next {
  + .x {
    @slot;
  }
}
```

### Candidates

```text
hv:flex op:flex child:flex either:flex blk:flex blk-next:flex
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  .hv\:flex :hover {
    display: flex;
  }
  .op\:flex .open {
    display: flex;
  }
  .child\:flex > .item {
    display: flex;
  }
  .either\:flex :hover, .either\:flex  .open {
    display: flex;
  }
  .blk\:flex :hover {
    display: flex;
  }
  .blk-next\:flex + .x {
    display: flex;
  }
}
```

A custom variant selector without `&` styles descendants, as in CSS Nesting. Stillwind
rejects `not-*` of these variants; `variants.negation` describes this difference.
