# Backgrounds, gradients, masks, and color opacity

## `backgrounds.gradients-and-images`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
bg-red-500/50 bg-linear-to-r from-red-500 via-white to-blue-500 bg-cover bg-center bg-fixed bg-clip-text text-transparent bg-[url(/hero.svg)] mask-linear-from-0 mask-linear-to-100
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer properties;
@layer theme {
  :root, :host {
    --color-red-500: oklch(63.7% 0.237 25.331);
    --color-blue-500: oklch(62.3% 0.214 259.815);
    --color-white: #fff;
    --spacing: 0.25rem;
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

## `backgrounds.gradient-arbitrary-positions`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
bg-linear-(--x) bg-linear-[to_right,red,blue] bg-conic-(--x) bg-radial-[at_center]
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  .bg-conic-\(--x\) {
    --tw-gradient-position: var(--x);
    background-image: conic-gradient(var(--tw-gradient-stops,var(--x)));
  }
  .bg-linear-\(--x\) {
    --tw-gradient-position: var(--x);
    background-image: linear-gradient(var(--tw-gradient-stops,var(--x)));
  }
  .bg-linear-\[to_right\,red\,blue\] {
    --tw-gradient-position: to right,red,blue;
    background-image: linear-gradient(var(--tw-gradient-stops,to right,red,blue));
  }
  .bg-radial-\[at_center\] {
    --tw-gradient-position: at center;
    background-image: radial-gradient(var(--tw-gradient-stops,at center));
  }
}
```
