# Cross-property cascade interactions

These cases are intentionally interpreted as class lists applied to one
element. They cover declarations whose cascade order can change computed
values, plus compound utilities that must retain all of their declarations.

## `interactions.transforms-and-filters`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
transform-gpu translate-x-4 -translate-y-2 rotate-45 scale-110 skew-x-6 origin-top-left blur-sm brightness-125 filter-none backdrop-blur-sm
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --spacing: 0.25rem;
    --blur-sm: 8px;
  }
}
@layer utilities {
  .origin-top-left {
    transform-origin: 0 0;
  }
  .translate-x-4 {
    --tw-translate-x: calc(var(--spacing) * 4);
    translate: var(--tw-translate-x) var(--tw-translate-y);
  }
  .-translate-y-2 {
    --tw-translate-y: calc(var(--spacing) * -2);
    translate: var(--tw-translate-x) var(--tw-translate-y);
  }
  .scale-110 {
    --tw-scale-x: 110%;
    --tw-scale-y: 110%;
    --tw-scale-z: 110%;
    scale: var(--tw-scale-x) var(--tw-scale-y);
  }
  .rotate-45 {
    rotate: 45deg;
  }
  .skew-x-6 {
    --tw-skew-x: skewX(6deg);
    transform: var(--tw-rotate-x,) var(--tw-rotate-y,) var(--tw-rotate-z,) var(--tw-skew-x,) var(--tw-skew-y,);
  }
  .transform-gpu {
    transform: translateZ(0) var(--tw-rotate-x,) var(--tw-rotate-y,) var(--tw-rotate-z,) var(--tw-skew-x,) var(--tw-skew-y,);
  }
  .blur-sm {
    --tw-blur: blur(var(--blur-sm));
    filter: var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);
  }
  .brightness-125 {
    --tw-brightness: brightness(125%);
    filter: var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);
  }
  .filter-none {
    filter: none;
  }
  .backdrop-blur-sm {
    --tw-backdrop-blur: blur(var(--blur-sm));
    -webkit-backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
    backdrop-filter: var(--tw-backdrop-blur,) var(--tw-backdrop-brightness,) var(--tw-backdrop-contrast,) var(--tw-backdrop-grayscale,) var(--tw-backdrop-hue-rotate,) var(--tw-backdrop-invert,) var(--tw-backdrop-opacity,) var(--tw-backdrop-saturate,) var(--tw-backdrop-sepia,);
  }
}
@property --tw-translate-x {
  syntax: "*";
  inherits: false;
  initial-value: 0;
}
@property --tw-translate-y {
  syntax: "*";
  inherits: false;
  initial-value: 0;
}
@property --tw-translate-z {
  syntax: "*";
  inherits: false;
  initial-value: 0;
}
@property --tw-scale-x {
  syntax: "*";
  inherits: false;
  initial-value: 1;
}
@property --tw-scale-y {
  syntax: "*";
  inherits: false;
  initial-value: 1;
}
@property --tw-scale-z {
  syntax: "*";
  inherits: false;
  initial-value: 1;
}
@property --tw-rotate-x {
  syntax: "*";
  inherits: false;
}
@property --tw-rotate-y {
  syntax: "*";
  inherits: false;
}
@property --tw-rotate-z {
  syntax: "*";
  inherits: false;
}
@property --tw-skew-x {
  syntax: "*";
  inherits: false;
}
@property --tw-skew-y {
  syntax: "*";
  inherits: false;
}
@property --tw-blur {
  syntax: "*";
  inherits: false;
}
@property --tw-brightness {
  syntax: "*";
  inherits: false;
}
@property --tw-contrast {
  syntax: "*";
  inherits: false;
}
@property --tw-grayscale {
  syntax: "*";
  inherits: false;
}
@property --tw-hue-rotate {
  syntax: "*";
  inherits: false;
}
@property --tw-invert {
  syntax: "*";
  inherits: false;
}
@property --tw-opacity {
  syntax: "*";
  inherits: false;
}
@property --tw-saturate {
  syntax: "*";
  inherits: false;
}
@property --tw-sepia {
  syntax: "*";
  inherits: false;
}
@property --tw-drop-shadow {
  syntax: "*";
  inherits: false;
}
@property --tw-drop-shadow-color {
  syntax: "*";
  inherits: false;
}
@property --tw-drop-shadow-alpha {
  syntax: "<percentage>";
  inherits: false;
  initial-value: 100%;
}
@property --tw-drop-shadow-size {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-blur {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-brightness {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-contrast {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-grayscale {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-hue-rotate {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-invert {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-opacity {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-saturate {
  syntax: "*";
  inherits: false;
}
@property --tw-backdrop-sepia {
  syntax: "*";
  inherits: false;
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-translate-x: 0;
      --tw-translate-y: 0;
      --tw-translate-z: 0;
      --tw-scale-x: 1;
      --tw-scale-y: 1;
      --tw-scale-z: 1;
      --tw-rotate-x: initial;
      --tw-rotate-y: initial;
      --tw-rotate-z: initial;
      --tw-skew-x: initial;
      --tw-skew-y: initial;
      --tw-blur: initial;
      --tw-brightness: initial;
      --tw-contrast: initial;
      --tw-grayscale: initial;
      --tw-hue-rotate: initial;
      --tw-invert: initial;
      --tw-opacity: initial;
      --tw-saturate: initial;
      --tw-sepia: initial;
      --tw-drop-shadow: initial;
      --tw-drop-shadow-color: initial;
      --tw-drop-shadow-alpha: 100%;
      --tw-drop-shadow-size: initial;
      --tw-backdrop-blur: initial;
      --tw-backdrop-brightness: initial;
      --tw-backdrop-contrast: initial;
      --tw-backdrop-grayscale: initial;
      --tw-backdrop-hue-rotate: initial;
      --tw-backdrop-invert: initial;
      --tw-backdrop-opacity: initial;
      --tw-backdrop-saturate: initial;
      --tw-backdrop-sepia: initial;
    }
  }
}
```

## `interactions.background-shorthand-and-longhands`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
[background:red] bg-red-500 bg-linear-to-r from-blue-500 to-green-500 bg-center bg-cover bg-no-repeat bg-fixed bg-origin-border bg-clip-padding
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
    --color-green-500: oklch(72.3% 0.219 149.579);
    --color-blue-500: oklch(62.3% 0.214 259.815);
  }
}
@layer utilities {
  .bg-red-500 {
    background-color: var(--color-red-500);
  }
  .bg-linear-to-r {
    --tw-gradient-position: to right;
    @supports (background-image: linear-gradient(in lab, red, red)) {
      --tw-gradient-position: to right in oklab;
    }
    background-image: linear-gradient(var(--tw-gradient-stops));
  }
  .from-blue-500 {
    --tw-gradient-from: var(--color-blue-500);
    --tw-gradient-stops: var(--tw-gradient-via-stops, var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-to) var(--tw-gradient-to-position));
  }
  .to-green-500 {
    --tw-gradient-to: var(--color-green-500);
    --tw-gradient-stops: var(--tw-gradient-via-stops, var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-to) var(--tw-gradient-to-position));
  }
  .bg-cover {
    background-size: cover;
  }
  .bg-fixed {
    background-attachment: fixed;
  }
  .bg-clip-padding {
    background-clip: padding-box;
  }
  .bg-center {
    background-position: center;
  }
  .bg-no-repeat {
    background-repeat: no-repeat;
  }
  .bg-origin-border {
    background-origin: border-box;
  }
  .\[background\:red\] {
    background: red;
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
    }
  }
}
```

## `interactions.white-space-nowrap-and-text-wrap`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
whitespace-nowrap text-wrap
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  .text-wrap {
    text-wrap: wrap;
  }
  .whitespace-nowrap {
    white-space: nowrap;
  }
}
```

## `interactions.white-space-pre-line-and-text-nowrap`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
whitespace-pre-line text-nowrap
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  .text-nowrap {
    text-wrap: nowrap;
  }
  .whitespace-pre-line {
    white-space: pre-line;
  }
}
```

## `interactions.all-with-ordinary-utilities`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
[all:unset] m-4 mx-2 p-3 bg-red-500 border-2 rounded-lg opacity-50 transform-gpu
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
    --spacing: 0.25rem;
    --radius-lg: 0.5rem;
  }
}
@layer utilities {
  .m-4 {
    margin: calc(var(--spacing) * 4);
  }
  .mx-2 {
    margin-inline: calc(var(--spacing) * 2);
  }
  .transform-gpu {
    transform: translateZ(0) var(--tw-rotate-x,) var(--tw-rotate-y,) var(--tw-rotate-z,) var(--tw-skew-x,) var(--tw-skew-y,);
  }
  .rounded-lg {
    border-radius: var(--radius-lg);
  }
  .border-2 {
    border-style: var(--tw-border-style);
    border-width: 2px;
  }
  .bg-red-500 {
    background-color: var(--color-red-500);
  }
  .p-3 {
    padding: calc(var(--spacing) * 3);
  }
  .opacity-50 {
    opacity: 50%;
  }
  .\[all\:unset\] {
    all: unset;
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

## `interactions.flex-gap-and-alignment-shorthands`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
flex-1 grow shrink-0 basis-1/2 gap-8 gap-x-4 gap-y-2 place-content-center content-between justify-around place-items-center items-start justify-items-end
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
  .flex-1 {
    flex: 1;
  }
  .shrink-0 {
    flex-shrink: 0;
  }
  .grow {
    flex-grow: 1;
  }
  .basis-1\/2 {
    flex-basis: calc(1 / 2 * 100%);
  }
  .place-content-center {
    place-content: center;
  }
  .place-items-center {
    place-items: center;
  }
  .content-between {
    align-content: space-between;
  }
  .items-start {
    align-items: flex-start;
  }
  .justify-around {
    justify-content: space-around;
  }
  .justify-items-end {
    justify-items: end;
  }
  .gap-8 {
    gap: calc(var(--spacing) * 8);
  }
  .gap-x-4 {
    column-gap: calc(var(--spacing) * 4);
  }
  .gap-y-2 {
    row-gap: calc(var(--spacing) * 2);
  }
}
```

## `interactions.border-list-and-decoration-shorthands`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
[border:medium_solid_red] border-x-4 border-l-2 [list-style:square_inside] list-inside list-disc [text-decoration:underline_red] decoration-blue-500 decoration-2
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --color-blue-500: oklch(62.3% 0.214 259.815);
  }
}
@layer utilities {
  .list-inside {
    list-style-position: inside;
  }
  .list-disc {
    list-style-type: disc;
  }
  .border-x-4 {
    border-inline-style: var(--tw-border-style);
    border-inline-width: 4px;
  }
  .border-l-2 {
    border-left-style: var(--tw-border-style);
    border-left-width: 2px;
  }
  .decoration-blue-500 {
    text-decoration-color: var(--color-blue-500);
  }
  .decoration-2 {
    text-decoration-thickness: 2px;
  }
  .\[border\:medium_solid_red\] {
    border: medium solid red;
  }
  .\[list-style\:square_inside\] {
    list-style: square inside;
  }
  .\[text-decoration\:underline_red\] {
    text-decoration: underline red;
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

## `interactions.mask-shorthand-and-longhands`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
[mask:none] mask-[url(mask.svg)] mask-center mask-cover mask-no-repeat
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  .mask-\[url\(mask\.svg\)\] {
    mask-image: url(mask.svg);
  }
  .mask-cover {
    mask-size: cover;
  }
  .mask-center {
    mask-position: center;
  }
  .mask-no-repeat {
    mask-repeat: no-repeat;
  }
  .\[mask\:none\] {
    mask: none;
  }
}
```
