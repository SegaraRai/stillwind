# Accessibility, interaction, scrolling, and form appearance

## `interaction.platform-controls`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
sr-only not-sr-only pointer-events-none pointer-events-auto select-none select-text touch-pan-x cursor-pointer caret-red-500 accent-red-500 appearance-none resize-y scroll-smooth snap-x snap-mandatory overscroll-contain
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
  }
}
@layer utilities {
  .pointer-events-auto {
    pointer-events: auto;
  }
  .pointer-events-none {
    pointer-events: none;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
    border-width: 0;
  }
  .not-sr-only {
    position: static;
    width: auto;
    height: auto;
    padding: 0;
    margin: 0;
    overflow: visible;
    clip-path: none;
    white-space: normal;
  }
  .cursor-pointer {
    cursor: pointer;
  }
  .touch-pan-x {
    --tw-pan-x: pan-x;
    touch-action: var(--tw-pan-x,) var(--tw-pan-y,) var(--tw-pinch-zoom,);
  }
  .resize-y {
    resize: vertical;
  }
  .snap-x {
    scroll-snap-type: x var(--tw-scroll-snap-strictness);
  }
  .snap-mandatory {
    --tw-scroll-snap-strictness: mandatory;
  }
  .appearance-none {
    appearance: none;
  }
  .overscroll-contain {
    overscroll-behavior: contain;
  }
  .scroll-smooth {
    scroll-behavior: smooth;
  }
  .caret-red-500 {
    caret-color: var(--color-red-500);
  }
  .accent-red-500 {
    accent-color: var(--color-red-500);
  }
  .select-none {
    -webkit-user-select: none;
    user-select: none;
  }
  .select-text {
    -webkit-user-select: text;
    user-select: text;
  }
}
@property --tw-pan-x {
  syntax: "*";
  inherits: false;
}
@property --tw-pan-y {
  syntax: "*";
  inherits: false;
}
@property --tw-pinch-zoom {
  syntax: "*";
  inherits: false;
}
@property --tw-scroll-snap-strictness {
  syntax: "*";
  inherits: false;
  initial-value: proximity;
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-pan-x: initial;
      --tw-pan-y: initial;
      --tw-pinch-zoom: initial;
      --tw-scroll-snap-strictness: proximity;
    }
  }
}
```

## `interaction.scroll-spacing-property-interactions`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
scroll-m-40 scroll-mx-20 scroll-ml-10 scroll-p-40 scroll-px-20 scroll-pl-10
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
  .scroll-m-40 {
    scroll-margin: calc(var(--spacing) * 40);
  }
  .scroll-mx-20 {
    scroll-margin-inline: calc(var(--spacing) * 20);
  }
  .scroll-ml-10 {
    scroll-margin-left: calc(var(--spacing) * 10);
  }
  .scroll-p-40 {
    scroll-padding: calc(var(--spacing) * 40);
  }
  .scroll-px-20 {
    scroll-padding-inline: calc(var(--spacing) * 20);
  }
  .scroll-pl-10 {
    scroll-padding-left: calc(var(--spacing) * 10);
  }
}
```
