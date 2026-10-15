# Color fallbacks

## `color-fallback.opacity-boundaries`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
bg-red-500 bg-red-500/0 bg-red-500/50 bg-red-500/100 bg-current/0 bg-current/50 bg-current/100
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
  }
}
@layer utilities {
  .bg-current\/0 {
    background-color: currentcolor;
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, currentcolor 0%, transparent);
    }
  }
  .bg-current\/50 {
    background-color: currentcolor;
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, currentcolor 50%, transparent);
    }
  }
  .bg-current\/100 {
    background-color: currentcolor;
  }
  .bg-red-500 {
    background-color: var(--color-red-500);
  }
  .bg-red-500\/0 {
    background-color: color-mix(in srgb, oklch(63.7% 0.237 25.331) 0%, transparent);
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, var(--color-red-500) 0%, transparent);
    }
  }
  .bg-red-500\/50 {
    background-color: color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, var(--color-red-500) 50%, transparent);
    }
  }
  .bg-red-500\/100 {
    background-color: var(--color-red-500);
  }
}
```

## `color-fallback.literal-colors`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
bg-[red]/50 bg-[#ff0000]/50 bg-transparent/50 bg-[var(--color-red-500)]/50
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
  }
}
@layer utilities {
  .bg-\[\#ff0000\]\/50 {
    background-color: color-mix(in oklab, #ff0000 50%, transparent);
  }
  .bg-\[red\]\/50 {
    background-color: color-mix(in oklab, red 50%, transparent);
  }
  .bg-\[var\(--color-red-500\)\]\/50 {
    background-color: color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, var(--color-red-500) 50%, transparent);
    }
  }
  .bg-transparent\/50 {
    background-color: color-mix(in oklab, transparent 50%, transparent);
  }
}
```

## `color-fallback.runtime-values`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
bg-(--brand)/50 bg-(--brand)/100 bg-red-500/(--opacity) bg-[red]/(--opacity) bg-current/(--opacity) bg-(--brand)/(--opacity)
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
  }
}
@layer utilities {
  .bg-\(--brand\)\/\(--opacity\) {
    background-color: var(--brand);
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, var(--brand) var(--opacity), transparent);
    }
  }
  .bg-\(--brand\)\/50 {
    background-color: var(--brand);
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, var(--brand) 50%, transparent);
    }
  }
  .bg-\(--brand\)\/100 {
    background-color: var(--brand);
  }
  .bg-\[red\]\/\(--opacity\) {
    background-color: red;
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, red var(--opacity), transparent);
    }
  }
  .bg-current\/\(--opacity\) {
    background-color: currentcolor;
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, currentcolor var(--opacity), transparent);
    }
  }
  .bg-red-500\/\(--opacity\) {
    background-color: oklch(63.7% 0.237 25.331);
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, var(--color-red-500) var(--opacity), transparent);
    }
  }
}
```

## `color-fallback.theme-values`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);

@theme {
  --color-linked: var(--external);
  --color-alias: var(--color-red-500);
  --color-nested: color-mix(in oklab, var(--color-red-500) 50%, blue);
}
```

### Candidates

```text
bg-linked/50 bg-alias/50 bg-nested/50
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
    --color-linked: var(--external);
    --color-alias: var(--color-red-500);
    --color-nested: color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, blue);
    @supports (color: color-mix(in lab, red, red)) {
      --color-nested: color-mix(in oklab, var(--color-red-500) 50%, blue);
    }
  }
}
@layer utilities {
  .bg-alias\/50 {
    background-color: color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, var(--color-alias) 50%, transparent);
    }
  }
  .bg-linked\/50 {
    background-color: var(--color-linked);
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, var(--color-linked) 50%, transparent);
    }
  }
  .bg-nested\/50 {
    background-color: color-mix(in srgb, color-mix(in oklab, var(--color-red-500) 50%, blue) 50%, transparent);
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, var(--color-nested) 50%, transparent);
    }
  }
}
```

## `color-fallback.shadows-rings-gradients`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
shadow-red-500/50 shadow-[color:red]/50 shadow-(color:--brand)/50 ring-red-500/50 ring-[red]/50 ring-(color:--brand)/50 from-red-500/50 from-[red]/50 from-(--brand)/50
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
  .from-\(--brand\)\/50 {
    --tw-gradient-from: var(--brand);
    @supports (color: color-mix(in lab, red, red)) {
      --tw-gradient-from: color-mix(in oklab, var(--brand) 50%, transparent);
    }
    --tw-gradient-stops: var(--tw-gradient-via-stops, var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-to) var(--tw-gradient-to-position));
  }
  .from-\[red\]\/50 {
    --tw-gradient-from: color-mix(in oklab, red 50%, transparent);
    --tw-gradient-stops: var(--tw-gradient-via-stops, var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-to) var(--tw-gradient-to-position));
  }
  .from-red-500\/50 {
    --tw-gradient-from: color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);
    @supports (color: color-mix(in lab, red, red)) {
      --tw-gradient-from: color-mix(in oklab, var(--color-red-500) 50%, transparent);
    }
    --tw-gradient-stops: var(--tw-gradient-via-stops, var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-to) var(--tw-gradient-to-position));
  }
  .shadow-\(color\:--brand\)\/50 {
    --tw-shadow-color: var(--brand);
    @supports (color: color-mix(in lab, red, red)) {
      --tw-shadow-color: color-mix(in oklab, color-mix(in oklab, var(--brand) 50%, transparent) var(--tw-shadow-alpha), transparent);
    }
  }
  .shadow-\[color\:red\]\/50 {
    --tw-shadow-color: color-mix(in srgb, red 50%, transparent);
    @supports (color: color-mix(in lab, red, red)) {
      --tw-shadow-color: color-mix(in oklab, color-mix(in oklab, red 50%, transparent) var(--tw-shadow-alpha), transparent);
    }
  }
  .shadow-red-500\/50 {
    --tw-shadow-color: color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);
    @supports (color: color-mix(in lab, red, red)) {
      --tw-shadow-color: color-mix(in oklab, color-mix(in oklab, var(--color-red-500) 50%, transparent) var(--tw-shadow-alpha), transparent);
    }
  }
  .ring-\(color\:--brand\)\/50 {
    --tw-ring-color: var(--brand);
    @supports (color: color-mix(in lab, red, red)) {
      --tw-ring-color: color-mix(in oklab, var(--brand) 50%, transparent);
    }
  }
  .ring-\[red\]\/50 {
    --tw-ring-color: color-mix(in oklab, red 50%, transparent);
  }
  .ring-red-500\/50 {
    --tw-ring-color: color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);
    @supports (color: color-mix(in lab, red, red)) {
      --tw-ring-color: color-mix(in oklab, var(--color-red-500) 50%, transparent);
    }
  }
}
@property --tw-gradient-position {
  syntax: "*";
  inherits: false;
}
@property --tw-gradient-from {
  syntax: "<color>";
  inherits: false;
  initial-value: #0000;
}
@property --tw-gradient-via {
  syntax: "<color>";
  inherits: false;
  initial-value: #0000;
}
@property --tw-gradient-to {
  syntax: "<color>";
  inherits: false;
  initial-value: #0000;
}
@property --tw-gradient-stops {
  syntax: "*";
  inherits: false;
}
@property --tw-gradient-via-stops {
  syntax: "*";
  inherits: false;
}
@property --tw-gradient-from-position {
  syntax: "<length-percentage>";
  inherits: false;
  initial-value: 0%;
}
@property --tw-gradient-via-position {
  syntax: "<length-percentage>";
  inherits: false;
  initial-value: 50%;
}
@property --tw-gradient-to-position {
  syntax: "<length-percentage>";
  inherits: false;
  initial-value: 100%;
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
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-gradient-position: initial;
      --tw-gradient-from: #0000;
      --tw-gradient-via: #0000;
      --tw-gradient-to: #0000;
      --tw-gradient-stops: initial;
      --tw-gradient-via-stops: initial;
      --tw-gradient-from-position: 0%;
      --tw-gradient-via-position: 50%;
      --tw-gradient-to-position: 100%;
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
    }
  }
}
```

## `color-fallback.arbitrary-mixes`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
[color:color-mix(in_oklab,var(--brand)_50%,transparent)] [color:color-mix(in_oklab,color-mix(in_oklab,var(--external)_50%,blue)_50%,transparent)] [color:color-mix(in_oklab,color-mix(in_oklab,red_50%,blue)_50%,var(--external))]
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  .\[color\:color-mix\(in_oklab\,color-mix\(in_oklab\,red_50\%\,blue\)_50\%\,var\(--external\)\)\] {
    color: color-mix(in srgb,red 50%,blue);
    @supports (color: color-mix(in lab, red, red)) {
      color: color-mix(in oklab,color-mix(in oklab,red 50%,blue) 50%,var(--external));
    }
  }
  .\[color\:color-mix\(in_oklab\,color-mix\(in_oklab\,var\(--external\)_50\%\,blue\)_50\%\,transparent\)\] {
    color: var(--external);
    @supports (color: color-mix(in lab, red, red)) {
      color: color-mix(in oklab,color-mix(in oklab,var(--external) 50%,blue) 50%,transparent);
    }
  }
  .\[color\:color-mix\(in_oklab\,var\(--brand\)_50\%\,transparent\)\] {
    color: var(--brand);
    @supports (color: color-mix(in lab, red, red)) {
      color: color-mix(in oklab,var(--brand) 50%,transparent);
    }
  }
}
```

## `color-fallback.nested-media`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);

@utility tone-media {
  @media (width >= 1px) {
    color: color-mix(in oklab, var(--color-red-500) 50%, transparent);
  }
}
```

### Candidates

```text
tone-media
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
  }
}
@layer utilities {
  @media (width >= 1px) {
    .tone-media {
      color: color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);
    }
    @supports (color: color-mix(in lab, red, red)) {
      .tone-media {
        color: color-mix(in oklab, var(--color-red-500) 50%, transparent);
      }
    }
  }
}
```

## `color-fallback.nested-media-important`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);

@utility tone-media {
  @media (width >= 1px) {
    color: color-mix(in oklab, var(--color-red-500) 50%, transparent);
  }
}
```

### Candidates

```text
tone-media!
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
  }
}
@layer utilities {
  @media (width >= 1px) {
    .tone-media\! {
      color: color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent) !important;
    }
    @supports (color: color-mix(in lab, red, red)) {
      .tone-media\! {
        color: color-mix(in oklab, var(--color-red-500) 50%, transparent) !important;
      }
    }
  }
}
```

## `color-fallback.nested-supports`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);

@utility tone-supports {
  @supports (display: grid) {
    color: color-mix(in oklab, var(--color-red-500) 50%, transparent);
  }
}
```

### Candidates

```text
tone-supports
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
  }
}
@layer utilities {
  @supports (display: grid) {
    .tone-supports {
      color: color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);
    }
    @supports (color: color-mix(in lab, red, red)) {
      .tone-supports {
        color: color-mix(in oklab, var(--color-red-500) 50%, transparent);
      }
    }
  }
}
```

## `color-fallback.parent-with-child-selector`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);

@utility tone-parent {
  color: color-mix(in oklab, var(--color-red-500) 50%, transparent);
  &::before {
    display: block;
  }
}
```

### Candidates

```text
tone-parent
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
  }
}
@layer utilities {
  .tone-parent {
    color: color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);
    @supports (color: color-mix(in lab, red, red)) {
      color: color-mix(in oklab, var(--color-red-500) 50%, transparent);
    }
    &::before {
      display: block;
    }
  }
}
```
