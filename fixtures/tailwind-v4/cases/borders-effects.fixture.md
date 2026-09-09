# Borders, outlines, rings, shadows, and compositing

## `effects.borders-and-shadows`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
border border-red-500 border-x-2 divide-y divide-slate-200 rounded-full outline-2 outline-offset-2 -outline-offset-[1px] -underline-offset-[1px] ring-2 ring-red-500 shadow-lg shadow-red-500/50 opacity-50 mix-blend-multiply bg-blend-multiply
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
    --color-slate-200: oklch(92.9% 0.013 255.508);
  }
}
@layer utilities {
  :where(.divide-y > :not(:last-child)) {
    --tw-divide-y-reverse: 0;
    border-bottom-style: var(--tw-border-style);
    border-top-style: var(--tw-border-style);
    border-top-width: calc(1px * var(--tw-divide-y-reverse));
    border-bottom-width: calc(1px * calc(1 - var(--tw-divide-y-reverse)));
  }
  :where(.divide-slate-200 > :not(:last-child)) {
    border-color: var(--color-slate-200);
  }
  .rounded-full {
    border-radius: calc(infinity * 1px);
  }
  .border {
    border-style: var(--tw-border-style);
    border-width: 1px;
  }
  .border-x-2 {
    border-inline-style: var(--tw-border-style);
    border-inline-width: 2px;
  }
  .border-red-500 {
    border-color: var(--color-red-500);
  }
  .-underline-offset-\[1px\] {
    text-underline-offset: calc(1px * -1);
  }
  .opacity-50 {
    opacity: 50%;
  }
  .bg-blend-multiply {
    background-blend-mode: multiply;
  }
  .mix-blend-multiply {
    mix-blend-mode: multiply;
  }
  .shadow-lg {
    --tw-shadow: 0 10px 15px -3px var(--tw-shadow-color, rgb(0 0 0 / 0.1)), 0 4px 6px -4px var(--tw-shadow-color, rgb(0 0 0 / 0.1));
    box-shadow: var(--tw-inset-shadow), var(--tw-inset-ring-shadow), var(--tw-ring-offset-shadow), var(--tw-ring-shadow), var(--tw-shadow);
  }
  .ring-2 {
    --tw-ring-shadow: var(--tw-ring-inset,) 0 0 0 calc(2px + var(--tw-ring-offset-width)) var(--tw-ring-color, currentcolor);
    box-shadow: var(--tw-inset-shadow), var(--tw-inset-ring-shadow), var(--tw-ring-offset-shadow), var(--tw-ring-shadow), var(--tw-shadow);
  }
  .shadow-red-500\/50 {
    --tw-shadow-color: color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);
    @supports (color: color-mix(in lab, red, red)) {
      --tw-shadow-color: color-mix(in oklab, color-mix(in oklab, var(--color-red-500) 50%, transparent) var(--tw-shadow-alpha), transparent);
    }
  }
  .ring-red-500 {
    --tw-ring-color: var(--color-red-500);
  }
  .outline-2 {
    outline-style: var(--tw-outline-style);
    outline-width: 2px;
  }
  .-outline-offset-\[1px\] {
    outline-offset: calc(1px * -1);
  }
  .outline-offset-2 {
    outline-offset: 2px;
  }
}
@property --tw-divide-y-reverse {
  syntax: "*";
  inherits: false;
  initial-value: 0;
}
@property --tw-border-style {
  syntax: "*";
  inherits: false;
  initial-value: solid;
}
@property --tw-shadow {
  syntax: "*";
  inherits: false;
  initial-value: 0 0 #0000;
}
@property --tw-shadow-color {
  syntax: "*";
  inherits: false;
}
@property --tw-shadow-alpha {
  syntax: "<percentage>";
  inherits: false;
  initial-value: 100%;
}
@property --tw-inset-shadow {
  syntax: "*";
  inherits: false;
  initial-value: 0 0 #0000;
}
@property --tw-inset-shadow-color {
  syntax: "*";
  inherits: false;
}
@property --tw-inset-shadow-alpha {
  syntax: "<percentage>";
  inherits: false;
  initial-value: 100%;
}
@property --tw-ring-color {
  syntax: "*";
  inherits: false;
}
@property --tw-ring-shadow {
  syntax: "*";
  inherits: false;
  initial-value: 0 0 #0000;
}
@property --tw-inset-ring-color {
  syntax: "*";
  inherits: false;
}
@property --tw-inset-ring-shadow {
  syntax: "*";
  inherits: false;
  initial-value: 0 0 #0000;
}
@property --tw-ring-inset {
  syntax: "*";
  inherits: false;
}
@property --tw-ring-offset-width {
  syntax: "<length>";
  inherits: false;
  initial-value: 0px;
}
@property --tw-ring-offset-color {
  syntax: "*";
  inherits: false;
  initial-value: #fff;
}
@property --tw-ring-offset-shadow {
  syntax: "*";
  inherits: false;
  initial-value: 0 0 #0000;
}
@property --tw-outline-style {
  syntax: "*";
  inherits: false;
  initial-value: solid;
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-divide-y-reverse: 0;
      --tw-border-style: solid;
      --tw-shadow: 0 0 #0000;
      --tw-shadow-color: initial;
      --tw-shadow-alpha: 100%;
      --tw-inset-shadow: 0 0 #0000;
      --tw-inset-shadow-color: initial;
      --tw-inset-shadow-alpha: 100%;
      --tw-ring-color: initial;
      --tw-ring-shadow: 0 0 #0000;
      --tw-inset-ring-color: initial;
      --tw-inset-ring-shadow: 0 0 #0000;
      --tw-ring-inset: initial;
      --tw-ring-offset-width: 0px;
      --tw-ring-offset-color: #fff;
      --tw-ring-offset-shadow: 0 0 #0000;
      --tw-outline-style: solid;
    }
  }
}
```

## `borders.shorthand-logical-physical-interactions`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
border-8 border-x-4 border-l-2 rounded-3xl rounded-s-xl rounded-ss-sm
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --radius-sm: 0.25rem;
    --radius-xl: 0.75rem;
    --radius-3xl: 1.5rem;
  }
}
@layer utilities {
  .rounded-3xl {
    border-radius: var(--radius-3xl);
  }
  .rounded-s-xl {
    border-start-start-radius: var(--radius-xl);
    border-end-start-radius: var(--radius-xl);
  }
  .rounded-ss-sm {
    border-start-start-radius: var(--radius-sm);
  }
  .border-8 {
    border-style: var(--tw-border-style);
    border-width: 8px;
  }
  .border-x-4 {
    border-inline-style: var(--tw-border-style);
    border-inline-width: 4px;
  }
  .border-l-2 {
    border-left-style: var(--tw-border-style);
    border-left-width: 2px;
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
