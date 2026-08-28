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
