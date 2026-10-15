# Candidate grammar, arbitrary values, modifiers, and arbitrary variants

## `candidate.grammar-and-arbitrary-values`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
!flex flex! -mt-4 w-(--sidebar) [--sidebar:16rem] [color:rebeccapurple] [@supports(display:grid)]:grid supports-[display:grid]:grid content-['hello']
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --spacing: 0.25rem;
  }
}
@layer utilities {
  .-mt-4 {
    margin-top: calc(var(--spacing) * -4);
  }
  .\!flex {
    display: flex !important;
  }
  .flex\! {
    display: flex !important;
  }
  .w-\(--sidebar\) {
    width: var(--sidebar);
  }
  .\[color\:rebeccapurple\] {
    color: rebeccapurple;
  }
  .content-\[\'hello\'\] {
    --tw-content: 'hello';
    content: var(--tw-content);
  }
  .\[--sidebar\:16rem\] {
    --sidebar: 16rem;
  }
  @supports (display:grid) {
    .supports-\[display\:grid\]\:grid {
      display: grid;
    }
    .\[\@supports\(display\:grid\)\]\:grid {
      display: grid;
    }
  }
}
@property --tw-content {
  syntax: "*";
  inherits: false;
  initial-value: "";
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-content: "";
    }
  }
}
```

## `candidate.typed-arbitrary-values-modifiers-and-escaping`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
text-[length:1.25rem] text-(length:--font-size) bg-red-500/50 bg-red-500/[.35] text-sm/6 w-1/2 grid-cols-[200px_1fr] content-['hello_world'] content-['hello\_world'] -translate-x-[calc(100%_-_1rem)] [&:nth-child(3)]:bg-red-500
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
    --spacing: 0.25rem;
    --text-sm: 0.875rem;
  }
}
@layer utilities {
  .w-1\/2 {
    width: calc(1 / 2 * 100%);
  }
  .-translate-x-\[calc\(100\%_-_1rem\)\] {
    --tw-translate-x: calc(calc(100% - 1rem) * -1);
    translate: var(--tw-translate-x) var(--tw-translate-y);
  }
  .grid-cols-\[200px_1fr\] {
    grid-template-columns: 200px 1fr;
  }
  .bg-red-500\/50 {
    background-color: color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, var(--color-red-500) 50%, transparent);
    }
  }
  .bg-red-500\/\[\.35\] {
    background-color: color-mix(in srgb, oklch(63.7% 0.237 25.331) 35%, transparent);
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, var(--color-red-500) 35%, transparent);
    }
  }
  .text-sm\/6 {
    font-size: var(--text-sm);
    line-height: calc(var(--spacing) * 6);
  }
  .text-\(length\:--font-size\) {
    font-size: var(--font-size);
  }
  .text-\[length\:1\.25rem\] {
    font-size: 1.25rem;
  }
  .content-\[\'hello\\_world\'\] {
    --tw-content: 'hello_world';
    content: var(--tw-content);
  }
  .content-\[\'hello_world\'\] {
    --tw-content: 'hello world';
    content: var(--tw-content);
  }
  .\[\&\:nth-child\(3\)\]\:bg-red-500:nth-child(3) {
    background-color: var(--color-red-500);
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
@property --tw-content {
  syntax: "*";
  inherits: false;
  initial-value: "";
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
      --tw-translate-x: 0;
      --tw-translate-y: 0;
      --tw-translate-z: 0;
      --tw-content: "";
    }
  }
}
```

## `candidate.untyped-variables`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
border-(--x) border-[var(--x)] from-(--x) via-(--x) to-(--x) text-(--x) text-[var(--x)] font-(--x) stroke-(--x) shadow-(--x) shadow-[var(--x)] drop-shadow-(--x) inset-shadow-(--x) transition-(--x) font-[500] font-light inset-shadow-[0_1px_red] transition-[opacity]
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --font-weight-light: 300;
    --default-transition-duration: 150ms;
    --default-transition-timing-function: cubic-bezier(0.4, 0, 0.2, 1);
  }
}
@layer utilities {
  .border-\(--x\) {
    border-color: var(--x);
  }
  .border-\[var\(--x\)\] {
    border-color: var(--x);
  }
  .from-\(--x\) {
    --tw-gradient-from: var(--x);
    --tw-gradient-stops: var(--tw-gradient-via-stops, var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-to) var(--tw-gradient-to-position));
  }
  .via-\(--x\) {
    --tw-gradient-via: var(--x);
    --tw-gradient-via-stops: var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-via) var(--tw-gradient-via-position), var(--tw-gradient-to) var(--tw-gradient-to-position);
    --tw-gradient-stops: var(--tw-gradient-via-stops);
  }
  .to-\(--x\) {
    --tw-gradient-to: var(--x);
    --tw-gradient-stops: var(--tw-gradient-via-stops, var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-to) var(--tw-gradient-to-position));
  }
  .stroke-\(--x\) {
    stroke: var(--x);
  }
  .font-\(--x\) {
    --tw-font-weight: var(--x);
    font-weight: var(--x);
  }
  .font-\[500\] {
    --tw-font-weight: 500;
    font-weight: 500;
  }
  .font-light {
    --tw-font-weight: var(--font-weight-light);
    font-weight: var(--font-weight-light);
  }
  .text-\(--x\) {
    color: var(--x);
  }
  .text-\[var\(--x\)\] {
    color: var(--x);
  }
  .shadow-\(--x\) {
    --tw-shadow: var(--x);
    box-shadow: var(--tw-inset-shadow), var(--tw-inset-ring-shadow), var(--tw-ring-offset-shadow), var(--tw-ring-shadow), var(--tw-shadow);
  }
  .shadow-\[var\(--x\)\] {
    --tw-shadow: var(--x);
    box-shadow: var(--tw-inset-shadow), var(--tw-inset-ring-shadow), var(--tw-ring-offset-shadow), var(--tw-ring-shadow), var(--tw-shadow);
  }
  .inset-shadow-\(--x\) {
    --tw-inset-shadow: inset var(--x);
    box-shadow: var(--tw-inset-shadow), var(--tw-inset-ring-shadow), var(--tw-ring-offset-shadow), var(--tw-ring-shadow), var(--tw-shadow);
  }
  .inset-shadow-\[0_1px_red\] {
    --tw-inset-shadow: inset 0 1px var(--tw-inset-shadow-color, red);
    box-shadow: var(--tw-inset-shadow), var(--tw-inset-ring-shadow), var(--tw-ring-offset-shadow), var(--tw-ring-shadow), var(--tw-shadow);
  }
  .drop-shadow-\(--x\) {
    --tw-drop-shadow-size: drop-shadow(var(--x));
    --tw-drop-shadow: var(--tw-drop-shadow-size);
    filter: var(--tw-blur,) var(--tw-brightness,) var(--tw-contrast,) var(--tw-grayscale,) var(--tw-hue-rotate,) var(--tw-invert,) var(--tw-saturate,) var(--tw-sepia,) var(--tw-drop-shadow,);
  }
  .transition-\(--x\) {
    transition-property: var(--x);
    transition-timing-function: var(--tw-ease, var(--default-transition-timing-function));
    transition-duration: var(--tw-duration, var(--default-transition-duration));
  }
  .transition-\[opacity\] {
    transition-property: opacity;
    transition-timing-function: var(--tw-ease, var(--default-transition-timing-function));
    transition-duration: var(--tw-duration, var(--default-transition-duration));
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
@property --tw-font-weight {
  syntax: "*";
  inherits: false;
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
      --tw-font-weight: initial;
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
    }
  }
}
```

## `candidate.theme-function-in-arbitrary-properties`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
[--x:theme(--color-red-500)]
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  .\[--x\:theme\(--color-red-500\)\] {
    --x: oklch(63.7% 0.237 25.331);
  }
}
```

## `candidate.arbitrary-value-functions-and-typed-variables`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
p-[calc(1rem+2px)] m-[calc(100%-2*var(--x))] bg-[theme(--color-red-500)] w-[theme(--spacing)] bg-red-500/[50%] font-[family-name:var(--x)] outline-(length:--x) outline-(--x) outline-[3px] ring-(length:--x) ring-(--x) ring-[3px] decoration-(length:--x) decoration-(--x) decoration-[3px] rotate-(--x) -rotate-(--x) rotate-[45deg] duration-(--x) duration-[2s]
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
  .m-\[calc\(100\%-2\*var\(--x\)\)\] {
    margin: calc(100% - 2 * var(--x));
  }
  .w-\[theme\(--spacing\)\] {
    width: 0.25rem;
  }
  .-rotate-\(--x\) {
    rotate: calc(var(--x) * -1);
  }
  .rotate-\(--x\) {
    rotate: var(--x);
  }
  .rotate-\[45deg\] {
    rotate: 45deg;
  }
  .bg-\[theme\(--color-red-500\)\] {
    background-color: oklch(63.7% 0.237 25.331);
  }
  .bg-red-500\/\[50\%\] {
    background-color: color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, var(--color-red-500) 50%, transparent);
    }
  }
  .p-\[calc\(1rem\+2px\)\] {
    padding: calc(1rem + 2px);
  }
  .font-\[family-name\:var\(--x\)\] {
    font-family: var(--x);
  }
  .decoration-\(--x\) {
    text-decoration-color: var(--x);
  }
  .decoration-\(length\:--x\) {
    text-decoration-thickness: var(--x);
  }
  .decoration-\[3px\] {
    text-decoration-thickness: 3px;
  }
  .ring-\(length\:--x\) {
    --tw-ring-shadow: var(--tw-ring-inset,) 0 0 0 calc(var(--x) + var(--tw-ring-offset-width)) var(--tw-ring-color, currentcolor);
    box-shadow: var(--tw-inset-shadow), var(--tw-inset-ring-shadow), var(--tw-ring-offset-shadow), var(--tw-ring-shadow), var(--tw-shadow);
  }
  .ring-\[3px\] {
    --tw-ring-shadow: var(--tw-ring-inset,) 0 0 0 calc(3px + var(--tw-ring-offset-width)) var(--tw-ring-color, currentcolor);
    box-shadow: var(--tw-inset-shadow), var(--tw-inset-ring-shadow), var(--tw-ring-offset-shadow), var(--tw-ring-shadow), var(--tw-shadow);
  }
  .ring-\(--x\) {
    --tw-ring-color: var(--x);
  }
  .outline-\(length\:--x\) {
    outline-style: var(--tw-outline-style);
    outline-width: var(--x);
  }
  .outline-\[3px\] {
    outline-style: var(--tw-outline-style);
    outline-width: 3px;
  }
  .outline-\(--x\) {
    outline-color: var(--x);
  }
  .duration-\(--x\) {
    --tw-duration: var(--x);
    transition-duration: var(--x);
  }
  .duration-\[2s\] {
    --tw-duration: 2s;
    transition-duration: 2s;
  }
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
@property --tw-duration {
  syntax: "*";
  inherits: false;
}
@layer properties {
  @supports ((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b)))) {
    *, ::before, ::after, ::backdrop {
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
      --tw-duration: initial;
    }
  }
}
```
