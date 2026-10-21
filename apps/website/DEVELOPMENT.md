# Website development

The Astro site uses the workspace Stillwind integration with `src/styles.css`.
The compiler generates displayed diagnostics and CSS at build time.

## Local tasks

Run from the repository root:

```sh
vp run @stillwind-internal/apps-website#dev
vp run @stillwind-internal/apps-website#build
vp run @stillwind-internal/apps-website#preview
vp run @stillwind-internal/apps-website#verify
```

Development and preview use port 4322. The build writes a static site to `dist/`.
The application supplies emnapi 1.x for Astro's checker; Stillwind's compiler
bindings use their own emnapi 2.x dependency.

## Presentation contracts

`src/lib/code-morph.ts` measures source tokens, and `src/lib/scroll.ts` supplies
normalized progress. CSS interpolates measured positions and visibility.
Only spaces and variant colons divide utility strings, so wrappers keep a
variant and its utility on one line. Reports identify utilities through source
ranges. Controllers release browser subscriptions when disposed.

`src/lib/features/` separates source examples, presentation, progress,
relationships, and motion. Examples use the real Astro macro APIs. Keep source,
explanation, and interactive preview consistent when editing an example.

Reduced motion and disabled JavaScript show examples in document order.
Viewports too short for pinning keep examples in normal flow. Code stays
selectable as the nearest complete source state. Manual motion playback retains
its final state until scrolling reaches the end, the stage leaves the viewport,
or a scene change hides it; those events restore scroll control.

## Fonts and measurements

Instrument Sans and JetBrains Mono are bundled for local delivery. Their licenses
live under `public/fonts/`. The code font's `zero` OpenType feature supplies
slashed zero and must remain available when replacing its file.
Code ligatures are disabled because token measurements must match displayed
punctuation. Font loading remeasures tokens, highlights, and connectors.
