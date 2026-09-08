# Built-in registry branch coverage

## `registry.static-and-relationship-branches`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
font-medium font-semibold position-area-block-end perspective-near perspective-midrange perspective-distant mask-radial size-screen @container/sidebar not-focus:bg-red-500
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
    --font-weight-medium: 500;
    --font-weight-semibold: 600;
    --perspective-near: 300px;
    --perspective-midrange: 800px;
    --perspective-distant: 1200px;
  }
}
@layer utilities {
  .\@container\/sidebar {
    container-type: inline-size;
    container-name: sidebar;
  }
  .font-medium {
    --tw-font-weight: var(--font-weight-medium);
    font-weight: var(--font-weight-medium);
  }
  .font-semibold {
    --tw-font-weight: var(--font-weight-semibold);
    font-weight: var(--font-weight-semibold);
  }
  .perspective-distant {
    perspective: var(--perspective-distant);
  }
  .perspective-midrange {
    perspective: var(--perspective-midrange);
  }
  .perspective-near {
    perspective: var(--perspective-near);
  }
  .not-focus\:bg-red-500:not(:focus) {
    background-color: var(--color-red-500);
  }
}
@property --tw-font-weight {
  syntax: "*";
  inherits: false;
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-font-weight: initial;
    }
  }
}
```

## `registry.typed-overload-branches`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);

@theme {
  --radius-audit: 0.625rem;
  --duration-audit: 175ms;
  --container-audit: 30rem;
}
```

### Candidates

```text
bg-[position:center_20%] bg-[bg-size:24rem_auto] border-[line-width:3px] border-x-[line-width:3px] border-y-[line-width:3px] border-s-[line-width:3px] border-e-[line-width:3px] border-bs-[line-width:3px] border-be-[line-width:3px] border-t-[line-width:3px] border-r-[line-width:3px] border-b-[line-width:3px] border-l-[line-width:3px] rounded-audit duration-audit mask-conic-45/oklab font-stretch-125 scale-[125%] bg-conic @min-audit:bg-red-500 @max-[48rem]:bg-red-500
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
    --radius-audit: 0.625rem;
  }
}
@layer utilities {
  .scale-\[125\%\] {
    scale: 125%;
  }
  .rounded-audit {
    border-radius: var(--radius-audit);
  }
  .border-\[line-width\:3px\] {
    border-style: var(--tw-border-style);
    border-width: 3px;
  }
  .border-x-\[line-width\:3px\] {
    border-inline-style: var(--tw-border-style);
    border-inline-width: 3px;
  }
  .border-y-\[line-width\:3px\] {
    border-block-style: var(--tw-border-style);
    border-block-width: 3px;
  }
  .border-s-\[line-width\:3px\] {
    border-inline-start-style: var(--tw-border-style);
    border-inline-start-width: 3px;
  }
  .border-e-\[line-width\:3px\] {
    border-inline-end-style: var(--tw-border-style);
    border-inline-end-width: 3px;
  }
  .border-bs-\[line-width\:3px\] {
    border-block-start-style: var(--tw-border-style);
    border-block-start-width: 3px;
  }
  .border-be-\[line-width\:3px\] {
    border-block-end-style: var(--tw-border-style);
    border-block-end-width: 3px;
  }
  .border-t-\[line-width\:3px\] {
    border-top-style: var(--tw-border-style);
    border-top-width: 3px;
  }
  .border-r-\[line-width\:3px\] {
    border-right-style: var(--tw-border-style);
    border-right-width: 3px;
  }
  .border-b-\[line-width\:3px\] {
    border-bottom-style: var(--tw-border-style);
    border-bottom-width: 3px;
  }
  .border-l-\[line-width\:3px\] {
    border-left-style: var(--tw-border-style);
    border-left-width: 3px;
  }
  .bg-conic {
    --tw-gradient-position: in oklab;
    background-image: conic-gradient(var(--tw-gradient-stops));
  }
  .bg-\[bg-size\:24rem_auto\] {
    background-size: 24rem auto;
  }
  .bg-\[position\:center_20\%\] {
    background-position: center 20%;
  }
  @container (width < 48rem) {
    .\@max-\[48rem\]\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 30rem) {
    .\@min-audit\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
}
@property --tw-border-style {
  syntax: "*";
  inherits: false;
  initial-value: solid;
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-border-style: solid;
    }
  }
}
```
