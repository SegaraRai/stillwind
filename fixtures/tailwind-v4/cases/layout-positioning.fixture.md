# Positioning, box model, overflow, and object layout

## `layout.positioning-and-box-model`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
relative absolute sticky inset-0 top-4 -left-px z-10 z-[100] isolate overflow-hidden overflow-x-auto object-cover aspect-video box-border box-decoration-clone float-end clear-both
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --spacing: 0.25rem;
    --aspect-video: 16 / 9;
  }
}
@layer utilities {
  .absolute {
    position: absolute;
  }
  .relative {
    position: relative;
  }
  .sticky {
    position: sticky;
  }
  .inset-0 {
    inset: 0px;
  }
  .top-4 {
    top: calc(var(--spacing) * 4);
  }
  .-left-px {
    left: -1px;
  }
  .isolate {
    isolation: isolate;
  }
  .z-10 {
    z-index: 10;
  }
  .z-\[100\] {
    z-index: 100;
  }
  .float-end {
    float: inline-end;
  }
  .clear-both {
    clear: both;
  }
  .box-border {
    box-sizing: border-box;
  }
  .aspect-video {
    aspect-ratio: var(--aspect-video);
  }
  .overflow-hidden {
    overflow: hidden;
  }
  .overflow-x-auto {
    overflow-x: auto;
  }
  .box-decoration-clone {
    -webkit-box-decoration-break: clone;
    box-decoration-break: clone;
  }
  .object-cover {
    object-fit: cover;
  }
}
```

## `layout.inset-and-overflow-property-interactions`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
inset-40 inset-x-20 left-10 overflow-hidden overflow-x-auto
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
  .inset-40 {
    inset: calc(var(--spacing) * 40);
  }
  .inset-x-20 {
    inset-inline: calc(var(--spacing) * 20);
  }
  .left-10 {
    left: calc(var(--spacing) * 10);
  }
  .overflow-hidden {
    overflow: hidden;
  }
  .overflow-x-auto {
    overflow-x: auto;
  }
}
```
