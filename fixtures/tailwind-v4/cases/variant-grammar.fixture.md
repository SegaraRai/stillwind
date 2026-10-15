# Parameterized, compound, named, arbitrary, and composed variant grammar

## `variants.dynamic-roots-and-composition`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
*:bg-red-500 **:bg-red-500 [&>svg]:fill-red-500 not-hover:bg-red-500 group-hover:bg-red-500 group-hover/menu:bg-red-500 peer-hover:bg-red-500 peer-hover/field:bg-red-500 has-checked:bg-red-500 in-focus:bg-red-500 aria-busy:bg-red-500 aria-[sort=ascending]:bg-red-500 data-open:bg-red-500 data-[state=open]:bg-red-500 nth-3:bg-red-500 nth-last-3:bg-red-500 nth-of-type-3:bg-red-500 nth-last-of-type-3:bg-red-500 supports-[display:grid]:bg-red-500 min-sm:bg-red-500 max-2xl:bg-red-500 min-[48rem]:bg-red-500 max-[48rem]:bg-red-500 @3xs:bg-red-500 @max-2xl:bg-red-500 @[48rem]:bg-red-500 @min-[48rem]:bg-red-500 md:hover:bg-red-500 dark:group-hover:bg-red-500
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
  :is(.\*\:bg-red-500 > *) {
    background-color: var(--color-red-500);
  }
  :is(.\*\*\:bg-red-500 *) {
    background-color: var(--color-red-500);
  }
  .not-hover\:bg-red-500:not(:hover) {
    background-color: var(--color-red-500);
  }
  @media not (hover: hover) {
    .not-hover\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (hover: hover) {
    .group-hover\:bg-red-500:is(:where(.group):hover *) {
      background-color: var(--color-red-500);
    }
    .group-hover\/menu\:bg-red-500:is(:where(.group\/menu):hover *) {
      background-color: var(--color-red-500);
    }
    .peer-hover\:bg-red-500:is(:where(.peer):hover ~ *) {
      background-color: var(--color-red-500);
    }
    .peer-hover\/field\:bg-red-500:is(:where(.peer\/field):hover ~ *) {
      background-color: var(--color-red-500);
    }
  }
  :where(:focus) .in-focus\:bg-red-500 {
    background-color: var(--color-red-500);
  }
  .has-checked\:bg-red-500:has(:checked) {
    background-color: var(--color-red-500);
  }
  .aria-busy\:bg-red-500[aria-busy="true"] {
    background-color: var(--color-red-500);
  }
  .aria-\[sort\=ascending\]\:bg-red-500[aria-sort="ascending"] {
    background-color: var(--color-red-500);
  }
  .data-open\:bg-red-500[data-open] {
    background-color: var(--color-red-500);
  }
  .data-\[state\=open\]\:bg-red-500[data-state="open"] {
    background-color: var(--color-red-500);
  }
  .nth-3\:bg-red-500:nth-child(3) {
    background-color: var(--color-red-500);
  }
  .nth-last-3\:bg-red-500:nth-last-child(3) {
    background-color: var(--color-red-500);
  }
  .nth-of-type-3\:bg-red-500:nth-of-type(3) {
    background-color: var(--color-red-500);
  }
  .nth-last-of-type-3\:bg-red-500:nth-last-of-type(3) {
    background-color: var(--color-red-500);
  }
  @supports (display:grid) {
    .supports-\[display\:grid\]\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (width < 96rem) {
    .max-2xl\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (width < 48rem) {
    .max-\[48rem\]\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (width >= 40rem) {
    .min-sm\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (width >= 48rem) {
    .min-\[48rem\]\:bg-red-500 {
      background-color: var(--color-red-500);
    }
    @media (hover: hover) {
      .md\:hover\:bg-red-500:hover {
        background-color: var(--color-red-500);
      }
    }
  }
  @container (width < 42rem) {
    .\@max-2xl\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 16rem) {
    .\@3xs\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 48rem) {
    .\@\[48rem\]\:bg-red-500 {
      background-color: var(--color-red-500);
    }
    .\@min-\[48rem\]\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (prefers-color-scheme: dark) {
    @media (hover: hover) {
      .dark\:group-hover\:bg-red-500:is(:where(.group):hover *) {
        background-color: var(--color-red-500);
      }
    }
  }
  .\[\&\>svg\]\:fill-red-500 > svg {
    fill: var(--color-red-500);
  }
}
```

## `variants.named-aria-data-and-default-breakpoints`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
aria-busy:bg-red-500 aria-checked:bg-red-500 aria-disabled:bg-red-500 aria-expanded:bg-red-500 aria-hidden:bg-red-500 aria-pressed:bg-red-500 aria-readonly:bg-red-500 aria-required:bg-red-500 aria-selected:bg-red-500 sm:bg-red-500 md:bg-red-500 lg:bg-red-500 xl:bg-red-500 2xl:bg-red-500 @3xs:bg-red-500 @2xs:bg-red-500 @xs:bg-red-500 @sm:bg-red-500 @md:bg-red-500 @lg:bg-red-500 @xl:bg-red-500 @2xl:bg-red-500 @3xl:bg-red-500 @4xl:bg-red-500 @5xl:bg-red-500 @6xl:bg-red-500 @7xl:bg-red-500
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
  .aria-busy\:bg-red-500[aria-busy="true"] {
    background-color: var(--color-red-500);
  }
  .aria-checked\:bg-red-500[aria-checked="true"] {
    background-color: var(--color-red-500);
  }
  .aria-disabled\:bg-red-500[aria-disabled="true"] {
    background-color: var(--color-red-500);
  }
  .aria-expanded\:bg-red-500[aria-expanded="true"] {
    background-color: var(--color-red-500);
  }
  .aria-hidden\:bg-red-500[aria-hidden="true"] {
    background-color: var(--color-red-500);
  }
  .aria-pressed\:bg-red-500[aria-pressed="true"] {
    background-color: var(--color-red-500);
  }
  .aria-readonly\:bg-red-500[aria-readonly="true"] {
    background-color: var(--color-red-500);
  }
  .aria-required\:bg-red-500[aria-required="true"] {
    background-color: var(--color-red-500);
  }
  .aria-selected\:bg-red-500[aria-selected="true"] {
    background-color: var(--color-red-500);
  }
  @media (width >= 40rem) {
    .sm\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (width >= 48rem) {
    .md\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (width >= 64rem) {
    .lg\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (width >= 80rem) {
    .xl\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @media (width >= 96rem) {
    .\32 xl\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 16rem) {
    .\@3xs\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 18rem) {
    .\@2xs\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 20rem) {
    .\@xs\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 24rem) {
    .\@sm\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 28rem) {
    .\@md\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 32rem) {
    .\@lg\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 36rem) {
    .\@xl\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 42rem) {
    .\@2xl\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 48rem) {
    .\@3xl\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 56rem) {
    .\@4xl\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 64rem) {
    .\@5xl\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 72rem) {
    .\@6xl\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
  @container (width >= 80rem) {
    .\@7xl\:bg-red-500 {
      background-color: var(--color-red-500);
    }
  }
}
```

## `variants.supports-features-and-variable-breakpoints`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
supports-grid:flex supports-display:flex supports-backdrop-filter:flex supports-[--foo]:flex not-supports-grid:flex min-(--x):flex max-(--x):flex @min-(--x):flex @max-(--x):flex
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  @supports not (grid: var(--tw)) {
    .not-supports-grid\:flex {
      display: flex;
    }
  }
  @supports (backdrop-filter: var(--tw)) {
    .supports-backdrop-filter\:flex {
      display: flex;
    }
  }
  @supports (display: var(--tw)) {
    .supports-display\:flex {
      display: flex;
    }
  }
  @supports (grid: var(--tw)) {
    .supports-grid\:flex {
      display: flex;
    }
  }
  @supports (--foo: var(--tw)) {
    .supports-\[--foo\]\:flex {
      display: flex;
    }
  }
}
```

Stillwind rejects `supports-(--foo)` and `supports-[(--foo)]`. Tailwind emits them as
`@supports var(--foo)` and `@supports (--foo)`. CSS evaluates both conditions as
false, so this case does not compare these candidates.

## `variants.breakpoint-functions`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
min-[calc(100px+1rem)]:flex max-[calc(100px-1rem)]:flex min-[theme(--breakpoint-md)]:flex @min-[calc(10px+1rem)]:flex
```

### Expected CSS

```css
/*! tailwindcss v4.3.3 | MIT License | https://tailwindcss.com */
@layer utilities {
  @media (width < calc(100px - 1rem)) {
    .max-\[calc\(100px-1rem\)\]\:flex {
      display: flex;
    }
  }
  @media (width >= calc(100px + 1rem)) {
    .min-\[calc\(100px\+1rem\)\]\:flex {
      display: flex;
    }
  }
  @media (width >= 48rem) {
    .min-\[theme\(--breakpoint-md\)\]\:flex {
      display: flex;
    }
  }
  @container (width >= calc(10px + 1rem)) {
    .\@min-\[calc\(10px\+1rem\)\]\:flex {
      display: flex;
    }
  }
}
```

## `variants.invalid-arbitrary-selectors`

### Input CSS

```css
@import "tailwindcss/theme" layer(theme);
@import "tailwindcss/utilities" layer(utilities);
```

### Candidates

```text
[color:red;]:flex [&;]:flex group-[color:red;]:flex
```

### Expected CSS

```css
```

Stillwind rejects an arbitrary variant whose text is not a valid selector list,
including inside `group-*`, `peer-*`, `has-*`, `in-*`, and `not-*`. Tailwind wraps such
text in `:is()` or appends it to the class, so this case does not compare these
candidates:

- `[--x:]:flex` and `[color:]:flex` emit `:is(--x:)` and `:is(color:)`, which match
  no element.
- `group-[color:]:flex` emits `:is(:where(.group):is(color:) *)`, which matches no
  element.
- `[&:]:flex` emits the invalid selector `.\[\&\:\]\:flex:`.
