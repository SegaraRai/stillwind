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

## `custom-css.functional-utility-user-theme-namespaces`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
@theme {
  --size-4: 1rem;
  --size-lg-2: 3rem;
  --size-1_5: 6px;
  --size-2\.5: 10px;
  --size-1\/2: 50%;
  --size-x--alt: 2px;
  --size-y: 1px;
  --size-y--alt: 4px;
  --inset-x: 3px;
  --inset-shadow-x: 0 0 1px red;
  --font-weight-x: 900;
}
@theme inline {
  --size-in: 5px;
}
@utility box-* {
  width: --value(--size-*);
}
@utility box-lg-* {
  height: --value(--size-lg-*);
}
@utility tbox-* {
  min-width: --value(--size-*, [length]);
}
@utility alt-* {
  max-width: --value(--size-*--alt);
}
@utility ins-* {
  top: --value(--inset-*);
}
@utility fw-* {
  font-weight: --value(--font-*);
}
```

### Candidates

```text
box-4 box-lg-2 box-1.5 box-2.5 box-1/2 box-in box-3 box-[2px] box-(--v) box-lg-lg-2 tbox-4 tbox-[2px] alt-x alt-y ins-x ins-shadow-x fw-weight-x fw-sans
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --font-sans: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', 'Noto Sans', Arial,
    sans-serif, 'Apple Color Emoji', 'Segoe UI Emoji', 'Segoe UI Symbol', 'Noto Color Emoji';
    --size-4: 1rem;
    --size-lg-2: 3rem;
    --size-1_5: 6px;
    --size-2\.5: 10px;
    --size-y--alt: 4px;
    --inset-x: 3px;
  }
}
@layer utilities {
  .ins-x {
    top: var(--inset-x);
  }
  .box-lg-2 {
    height: var(--size-lg-2);
  }
  .box-1\.5 {
    width: var(--size-1_5);
  }
  .box-2\.5 {
    width: var(--size-2\.5);
  }
  .box-4 {
    width: var(--size-4);
  }
  .box-in {
    width: 5px;
  }
  .box-lg-2 {
    width: var(--size-lg-2);
  }
  .alt-y {
    max-width: var(--size-y--alt);
  }
  .tbox-4 {
    min-width: var(--size-4);
  }
  .tbox-\[2px\] {
    min-width: 2px;
  }
  .fw-sans {
    font-weight: var(--font-sans);
  }
}
```
