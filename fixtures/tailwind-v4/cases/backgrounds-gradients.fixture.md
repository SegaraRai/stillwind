# Backgrounds, gradients, masks, and color opacity

## `backgrounds.gradients-and-images`

### Input CSS

```css
@import "tailwindcss";
```

### Candidates

```text
bg-red-500/50 bg-linear-to-r from-red-500 via-white to-blue-500 bg-cover bg-center bg-fixed bg-clip-text text-transparent bg-[url(/hero.svg)] mask-linear-from-0 mask-linear-to-100
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme, base, components, utilities;
@layer theme {
  :root, :host {
    --font-sans: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue",
      "Noto Sans", Arial, sans-serif, "Apple Color Emoji", "Segoe UI Emoji",
      "Segoe UI Symbol", "Noto Color Emoji";
    --font-mono: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono",
      "Courier New", monospace;
    --color-red-500: oklch(63.7% 0.237 25.331);
    --color-blue-500: oklch(62.3% 0.214 259.815);
    --color-white: #fff;
    --spacing: 0.25rem;
    --default-font-family: var(--font-sans);
    --default-mono-font-family: var(--font-mono);
  }
}
@layer base {
  *, ::after, ::before, ::backdrop, ::file-selector-button {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
    border: 0 solid;
  }
  html, :host {
    line-height: 1.5;
    -webkit-text-size-adjust: 100%;
    tab-size: 4;
    font-family: var(--default-font-family, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", "Noto Sans", Arial, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji");
    font-feature-settings: var(--default-font-feature-settings, normal);
    font-variation-settings: var(--default-font-variation-settings, normal);
    -webkit-tap-highlight-color: transparent;
  }
  hr {
    height: 0;
    color: inherit;
    border-top-width: 1px;
  }
  abbr:where([title]) {
    -webkit-text-decoration: underline dotted;
    text-decoration: underline dotted;
  }
  h1, h2, h3, h4, h5, h6 {
    font-size: inherit;
    font-weight: inherit;
  }
  a {
    color: inherit;
    -webkit-text-decoration: inherit;
    text-decoration: inherit;
  }
  b, strong {
    font-weight: bolder;
  }
  code, kbd, samp, pre {
    font-family: var(--default-mono-font-family, ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace);
    font-feature-settings: var(--default-mono-font-feature-settings, normal);
    font-variation-settings: var(--default-mono-font-variation-settings, normal);
    font-size: 1em;
  }
  small {
    font-size: 80%;
  }
  sub, sup {
    font-size: 75%;
    line-height: 0;
    position: relative;
    vertical-align: baseline;
  }
  sub {
    bottom: -0.25em;
  }
  sup {
    top: -0.5em;
  }
  table {
    text-indent: 0;
    border-color: inherit;
    border-collapse: collapse;
  }
  :-moz-focusring:where(:not(iframe)) {
    outline: auto;
  }
  progress {
    vertical-align: baseline;
  }
  summary {
    display: list-item;
  }
  ol, ul, menu {
    list-style: none;
  }
  img, svg, video, canvas, audio, iframe, embed, object {
    display: block;
    vertical-align: middle;
  }
  img, video {
    max-width: 100%;
    height: auto;
  }
  button, input, select, optgroup, textarea, ::file-selector-button {
    font: inherit;
    font-feature-settings: inherit;
    font-variation-settings: inherit;
    letter-spacing: inherit;
    color: inherit;
    border-radius: 0;
    background-color: transparent;
    opacity: 1;
  }
  :where(select:is([multiple], [size])) optgroup {
    font-weight: bolder;
  }
  :where(select:is([multiple], [size])) optgroup option {
    padding-inline-start: 20px;
  }
  ::file-selector-button {
    margin-inline-end: 4px;
  }
  ::placeholder {
    opacity: 1;
  }
  @supports (not (-webkit-appearance: -apple-pay-button))  or (contain-intrinsic-size: 1px) {
    ::placeholder {
      color: currentcolor;
      @supports (color: color-mix(in lab, red, red)) {
        color: color-mix(in oklab, currentcolor 50%, transparent);
      }
    }
  }
  textarea {
    resize: vertical;
  }
  ::-webkit-search-decoration {
    -webkit-appearance: none;
  }
  ::-webkit-date-and-time-value {
    min-height: 1lh;
    text-align: inherit;
  }
  ::-webkit-datetime-edit {
    display: inline-flex;
  }
  ::-webkit-datetime-edit-fields-wrapper {
    padding: 0;
  }
  ::-webkit-datetime-edit, ::-webkit-datetime-edit-year-field, ::-webkit-datetime-edit-month-field, ::-webkit-datetime-edit-day-field, ::-webkit-datetime-edit-hour-field, ::-webkit-datetime-edit-minute-field, ::-webkit-datetime-edit-second-field, ::-webkit-datetime-edit-millisecond-field, ::-webkit-datetime-edit-meridiem-field {
    padding-block: 0;
  }
  ::-webkit-calendar-picker-indicator {
    line-height: 1;
  }
  :-moz-ui-invalid {
    box-shadow: none;
  }
  button, input:where([type="button"], [type="reset"], [type="submit"]), ::file-selector-button {
    appearance: button;
  }
  ::-webkit-inner-spin-button, ::-webkit-outer-spin-button {
    height: auto;
  }
  [hidden]:where(:not([hidden="until-found"])) {
    display: none !important;
  }
}
@layer utilities {
  .bg-red-500\/50 {
    background-color: color-mix(in srgb, oklch(63.7% 0.237 25.331) 50%, transparent);
    @supports (color: color-mix(in lab, red, red)) {
      background-color: color-mix(in oklab, var(--color-red-500) 50%, transparent);
    }
  }
  .bg-linear-to-r {
    --tw-gradient-position: to right;
    @supports (background-image: linear-gradient(in lab, red, red)) {
      --tw-gradient-position: to right in oklab;
    }
    background-image: linear-gradient(var(--tw-gradient-stops));
  }
  .bg-\[url\(\/hero\.svg\)\] {
    background-image: url(/hero.svg);
  }
  .from-red-500 {
    --tw-gradient-from: var(--color-red-500);
    --tw-gradient-stops: var(--tw-gradient-via-stops, var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-to) var(--tw-gradient-to-position));
  }
  .via-white {
    --tw-gradient-via: var(--color-white);
    --tw-gradient-via-stops: var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-via) var(--tw-gradient-via-position), var(--tw-gradient-to) var(--tw-gradient-to-position);
    --tw-gradient-stops: var(--tw-gradient-via-stops);
  }
  .to-blue-500 {
    --tw-gradient-to: var(--color-blue-500);
    --tw-gradient-stops: var(--tw-gradient-via-stops, var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-to) var(--tw-gradient-to-position));
  }
  .mask-linear-from-0 {
    mask-image: var(--tw-mask-linear), var(--tw-mask-radial), var(--tw-mask-conic);
    mask-composite: intersect;
    --tw-mask-linear-stops: var(--tw-mask-linear-position), var(--tw-mask-linear-from-color) var(--tw-mask-linear-from-position), var(--tw-mask-linear-to-color) var(--tw-mask-linear-to-position);
    --tw-mask-linear: linear-gradient(var(--tw-mask-linear-stops));
    --tw-mask-linear-from-position: 0px;
  }
  .mask-linear-to-100 {
    mask-image: var(--tw-mask-linear), var(--tw-mask-radial), var(--tw-mask-conic);
    mask-composite: intersect;
    --tw-mask-linear-stops: var(--tw-mask-linear-position), var(--tw-mask-linear-from-color) var(--tw-mask-linear-from-position), var(--tw-mask-linear-to-color) var(--tw-mask-linear-to-position);
    --tw-mask-linear: linear-gradient(var(--tw-mask-linear-stops));
    --tw-mask-linear-to-position: calc(var(--spacing) * 100);
  }
  .bg-cover {
    background-size: cover;
  }
  .bg-fixed {
    background-attachment: fixed;
  }
  .bg-clip-text {
    background-clip: text;
  }
  .bg-center {
    background-position: center;
  }
  .text-transparent {
    color: transparent;
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
@property --tw-mask-linear {
  syntax: "*";
  inherits: false;
  initial-value: linear-gradient(#fff, #fff);
}
@property --tw-mask-radial {
  syntax: "*";
  inherits: false;
  initial-value: linear-gradient(#fff, #fff);
}
@property --tw-mask-conic {
  syntax: "*";
  inherits: false;
  initial-value: linear-gradient(#fff, #fff);
}
@property --tw-mask-linear-position {
  syntax: "*";
  inherits: false;
  initial-value: 0deg;
}
@property --tw-mask-linear-from-position {
  syntax: "*";
  inherits: false;
  initial-value: 0%;
}
@property --tw-mask-linear-to-position {
  syntax: "*";
  inherits: false;
  initial-value: 100%;
}
@property --tw-mask-linear-from-color {
  syntax: "*";
  inherits: false;
  initial-value: black;
}
@property --tw-mask-linear-to-color {
  syntax: "*";
  inherits: false;
  initial-value: transparent;
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
      --tw-mask-linear: linear-gradient(#fff, #fff);
      --tw-mask-radial: linear-gradient(#fff, #fff);
      --tw-mask-conic: linear-gradient(#fff, #fff);
      --tw-mask-linear-position: 0deg;
      --tw-mask-linear-from-position: 0%;
      --tw-mask-linear-to-position: 100%;
      --tw-mask-linear-from-color: black;
      --tw-mask-linear-to-color: transparent;
    }
  }
}
```
