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
