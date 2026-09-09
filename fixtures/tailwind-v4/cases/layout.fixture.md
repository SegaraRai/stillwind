# Layout, flex, grid, spacing, and sizing

## `layout.flex-grid-spacing`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
grid grid-cols-3 gap-4 inline-flex items-center justify-between px-4 -mt-2 size-10 w-[calc(var(--gutter)*2)]
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
  .-mt-2 {
    margin-top: calc(var(--spacing) * -2);
  }
  .grid {
    display: grid;
  }
  .inline-flex {
    display: inline-flex;
  }
  .size-10 {
    width: calc(var(--spacing) * 10);
    height: calc(var(--spacing) * 10);
  }
  .w-\[calc\(var\(--gutter\)\*2\)\] {
    width: calc(var(--gutter) * 2);
  }
  .grid-cols-3 {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }
  .items-center {
    align-items: center;
  }
  .justify-between {
    justify-content: space-between;
  }
  .gap-4 {
    gap: calc(var(--spacing) * 4);
  }
  .px-4 {
    padding-inline: calc(var(--spacing) * 4);
  }
}
```

## `layout.box-model-property-interactions`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
m-40 mx-20 ml-10 p-40 px-20 pl-10 size-40 w-20 h-16 block-10 inline-8
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
  .m-40 {
    margin: calc(var(--spacing) * 40);
  }
  .mx-20 {
    margin-inline: calc(var(--spacing) * 20);
  }
  .ml-10 {
    margin-left: calc(var(--spacing) * 10);
  }
  .size-40 {
    width: calc(var(--spacing) * 40);
    height: calc(var(--spacing) * 40);
  }
  .h-16 {
    height: calc(var(--spacing) * 16);
  }
  .w-20 {
    width: calc(var(--spacing) * 20);
  }
  .p-40 {
    padding: calc(var(--spacing) * 40);
  }
  .px-20 {
    padding-inline: calc(var(--spacing) * 20);
  }
  .pl-10 {
    padding-left: calc(var(--spacing) * 10);
  }
  .block-10 {
    block-size: calc(var(--spacing) * 10);
  }
  .inline-8 {
    inline-size: calc(var(--spacing) * 8);
  }
}
```
