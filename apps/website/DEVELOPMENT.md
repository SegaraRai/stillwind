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
The build runs after the Astro checker because both processes write Nimbus's
prepared content under `.astro/nimbus/`.
The application supplies emnapi 1.x for Astro's checker; Stillwind's compiler
bindings use their own emnapi 2.x dependency.

## Documentation

Nimbus reads the `docs` collection from `src/content/docs/`. The `docs/` folder
inside that collection maps to `/docs/`, while `src/pages/index.astro` owns `/`.
The document layout uses the website's Stillwind theme. Nimbus supplies content
validation, navigation, Markdown conversion, and agent indexes.

Use MDX for pages with Nimbus components. `src/components.ts` registers the local
Aside component, which supports `note`, `tip`, `caution`, and `danger`. Its styles
use the shared theme through `src/styles/docs.css`.

`src/styles/docs.css` uses `@apply` and is imported by the Stillwind stylesheet
entry, `src/styles.css`. Import styles with Stillwind directives through that
entry so the compiler expands them before Astro bundles the generated CSS.

Each page has an `index.md` alternate and an `index.mdx` source alternate.
`/llms.txt` links to `/docs/llms.txt`, which lists the documentation pages.
`/llms-full.txt` contains all discoverable documentation. These endpoints use
Nimbus's helpers and the same content collection as HTML.

`tests/documentation.test.ts` checks the built HTML, Markdown, indexes, and code
examples. The website test task and the root JavaScript test task depend on the
website build.

Set `SITE_URL` to the deployment origin before building for publication. It
defaults to `http://localhost:4322` for local previews and controls absolute
links in metadata, agent indexes, and the sitemap. Build and content-sync task
caches include this variable.

## Theme

`src/styles/theme.css` defines the shared theme with blue neutrals and indigo actions.
Its static tokens remain available to authored CSS and compiled examples. Use
semantic colors such as `bg-background`, `bg-surface`, `text-foreground`,
`text-muted`, `border-border`, and `bg-action text-action-foreground`. Use `accent`
for focus indicators and `danger`, `warning`, and `success` for status. Each color
uses `light-dark()` so its public name stays the same in both schemes.

The header cycles through system, light, and dark preferences. System follows
the browser's color scheme, including when JavaScript is disabled. Explicit
choices persist in local storage and apply in the head before page content.

| Tokens                                                                    | Contract                                                        |
| ------------------------------------------------------------------------- | --------------------------------------------------------------- |
| `--spacing`                                                               | A 4px base at the default root size; half steps supply 2px gaps |
| `gutter`, `columns`, `control`, `panel`, `stack`, `section` spacing       | Page padding, column gaps, and shared component spacing         |
| `inline`, `control`, `panel` radii                                        | 3px for inline code, 4px for controls, 8px for panels           |
| `inline` breakpoint                                                       | 40rem: horizontal groups and larger navigation                  |
| `sidebar` breakpoint                                                      | 52.5rem: side navigation and paired content                     |
| `outline` breakpoint                                                      | 70rem: the page outline and full demonstration layout           |
| `label`, `caption`, `ui`, `small`, `body`, `lead` text                    | 10, 12, 13, 14, 16, and 18px at the default root size           |
| `heading-small`, `heading`, `title`, `section`, `display` text            | Heading roles; large titles scale within defined limits         |
| `code`, `code-inline`, `code-display`, `code-output`, `code-feature` text | Reading and demonstration code roles                            |

The `demo` breakpoint is 20rem, the minimum width for interactive demonstrations.
Use named text sizes and nested `@variant` blocks in authored CSS rules.
Use named variants such as `sidebar:flex-row` in macro inputs. Presentation
controllers pass queries from `style.media()` to `matchMedia()`.
The `pin-story*` and `pin-features*` variants define each demonstration's minimum height.
Height and motion conditions govern whether demonstrations can pin and animate;
their thresholds are independent of the layout widths. Measured animation
coordinates and illustration dimensions remain local to their presentation.

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
