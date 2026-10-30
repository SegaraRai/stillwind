---
title: Why Stillwind
slug: docs/why
description: Keep Tailwind-style code consistent as AI agents write more of it.
---

Tailwind CSS makes styles fast to write, and you can write its classes anywhere.
As AI agents write more of your code, it becomes harder to keep theme values,
breakpoints, and names shared between elements consistent by hand.

Stillwind keeps the Tailwind CSS v4 utilities that you already know. It compiles
them at build time and checks them, so you can keep styling code in order.

## Keep values in your theme

A utility can produce the right appearance while using values outside your theme:

```tsx
style("bg-[#3b82f6] p-[13px] min-[713px]:flex");
```

You can enable rules that report arbitrary colors, spacing, and breakpoints.
Then every value has to come from the theme in your stylesheet:

```ts
lint: {
  extends: [lintPresets.recommended],
  rules: {
    "no-arbitrary-colors": "deny",
    "no-arbitrary-spacing": "deny",
    "no-arbitrary-breakpoints": "deny",
  },
},
```

To use a breakpoint in JavaScript, call [`style.media()`](/docs/macros/media) with its name. You do
not need to copy the breakpoint value into your code.

## Connect elements through variables

Tailwind connects elements by name: `group/card` on the parent and
`group-hover/card:` on the child. Tailwind reads these names as strings, so it
does not check that the two names agree.

In Stillwind, you create the name as a variable with [`style.ref()`](/docs/macros/ref):

```tsx
import { style } from "stillwind/react";

const card = style.ref("card");

export function Card() {
  return (
    <article {...style(card, "rounded-lg p-4")}>
      <h2 {...style({ [card("hover")]: "text-blue-600" })}>Project</h2>
    </article>
  );
}
```

A misspelled ref is an undefined variable, so TypeScript reports it. Each ref
gets a generated name, so refs with the same label in different components do
not affect each other.

## Define animations next to the component

With Tailwind, as with plain CSS, keyframes and view transition names live in a
global stylesheet. In a large codebase, it takes effort to find which component
uses which name.

In Stillwind, you define them in the component that uses them:

```tsx
import { style } from "stillwind/react";

const enter = style.keyframes({
  from: "opacity-0",
  to: "opacity-100",
});

export function Notice() {
  return <aside {...style(enter("duration-300"))}>Your changes are saved.</aside>;
}
```

The compiler generates a unique animation name. When you delete the component,
its animation goes with it.

## Override styles predictably

When you combine two styles, the later one wins:

```tsx
import { style } from "stillwind/react";

const button = style("rounded-md bg-blue-600 px-4 py-2 text-white");
const dangerButton = style(button, "bg-red-600");
```

`dangerButton` is always red. The compiler fixes the order of the generated CSS,
so the result does not depend on the order of class names or on how the bundler
loads modules. You do not need `tailwind-merge`.

## Check styles in the editor and the CLI

The CLI reports unknown utilities, conflicting utilities such as `p-2 p-4`, and
the rules that you enable. It also fixes and sorts utilities:

```sh
stillwind lint
stillwind lint --fix
stillwind fmt
```

AI agents can run the same commands as you, and CI can run them on every change.
The VS Code extension shows the same findings while you edit.

## Ship static CSS

The compiler turns static utilities into CSS files and class names at build time.
These styles need no JavaScript at runtime. Conditions and dynamic values add
only the small runtime helpers that they use. See
[How styles compile](/docs/concepts/how-styles-compile) for the generated code.

## What changes from Tailwind

Stillwind supports Tailwind CSS v4 utilities, variants, and plugins. You write
them in a different place:

- You pass utilities to `style()` and spread the result onto the element,
  instead of writing a `className` string.
- Stillwind needs a build tool. It supports Vite, Astro, Next.js, SvelteKit,
  Nuxt, webpack, Rspack, Rsbuild, and Turbopack.
- Some Tailwind settings move to `stillwind.config.ts`.

Tailwind CSS remains the better fit when a project has no build tool, or when
classes come from places that Stillwind does not compile, such as HTML from a
CMS.

[Getting started](/docs) adds Stillwind to a new application.
[Migrating from Tailwind CSS](/docs/guides/migrating-from-tailwind) moves an
existing Tailwind project.
