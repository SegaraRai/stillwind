# Website development

The Astro site uses the workspace Stillwind integration with `src/styles.css`.
The compiler generates displayed diagnostics and CSS at build time.

## Source layout

Code is grouped by the part of the site that uses it:

| Part    | Components                | Scripts and data   | Styles                   |
| ------- | ------------------------- | ------------------ | ------------------------ |
| Shared  | `src/components/site/`    | `src/lib/site/`    | `src/styles/site.css`    |
| Landing | `src/components/landing/` | `src/lib/landing/` | `src/styles/landing.css` |
| Docs    | `src/components/docs/`    | `src/lib/docs/`    | `src/styles/docs.css`    |

`src/styles/theme.css` defines the tokens for every part. `src/styles.css` is the
Stillwind stylesheet entry and imports the other stylesheets. Import styles with
Stillwind directives through that entry so the compiler expands them before Astro
bundles the generated CSS. Apply utilities with `style()` and share custom
utilities where server markup and browser code render the same elements. Attach
refs to connect presentation states across components. Keep `data-*` attributes
for browser hooks and states. Use `@apply` and `@variant` inside custom utilities
for reusable styles and generated content. Set inline values directly when no
utilities are involved. Presentation controllers pass measured values through
custom properties. Shared code must not import landing or docs code.

## Local tasks

Run from the repository root:

```sh
vp run @stillwind-internal/apps-website#dev
vp run @stillwind-internal/apps-website#build
vp run @stillwind-internal/apps-website#preview
vp run @stillwind-internal/apps-website#verify
```

Development and preview use port 4322. The build writes a static site to `dist/`.
See [Site deployments](../../DEVELOPMENT.md#site-deployments) for CI, Preview,
production, and the required Cloudflare and GitHub settings.
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

Each page has an `index.md` alternate and an `index.mdx` source alternate.
`/llms.txt` links to `/docs/llms.txt`, which lists the documentation pages.
`/llms-full.txt` contains all discoverable documentation. These endpoints use
Nimbus's helpers and the same content collection as HTML.

Each macro, lint rule, and compiler error has an authored page under
`src/content/docs/docs/`. Keep explanations, constraints, examples, and recovery
guidance in that page. `CompiledExample` renders its authored TypeScript code
fence together with actual compiler output. `LintRuleSummary` reads current preset
levels and option metadata after the rule's explanation and examples.
These components are registered in `src/components.ts` and have equivalent
Markdown renderers in `src/components/docs/reference/markdown.ts`. Edit links point
to the individual page sources.

Component revisions track compiler, theme, metadata, and renderer inputs for
Nimbus's Markdown cache.

## Utility lookup

`UtilityExplorer` on `/docs/utilities/` looks up utilities in the browser. Its
Markdown renderer outputs nothing, so the Markdown version and the LLM indexes
contain only the page's prose. `lookupUtilities()` in
`src/lib/docs/utilities/lookup.ts` reads the input in both directions. It resolves
utilities to their CSS and canonical spelling, converts each CSS declaration to an
arbitrary property and then to its canonical utility, and lists utilities that
start with a single word. When the canonical spelling of a utility generates
other declarations, the spelling as written follows it. `highlightCss()` uses
sugar-high to highlight the CSS of each result on the server and in the browser.

The build renders the results of the initial input with the native compiler.
In the browser, a worker runs the WASM compiler. `/docs/utilities/stylesheet.json`
contains the default stylesheet as the build recorded its load, and the worker
replays it into a design-system loader without resolving files. The threaded WASM
compiler needs a cross-origin isolated page. `public/_headers` sets the opener
and embedder policies for this page and the embedder policy for worker scripts.
Development and preview servers set both policies for every page.

`tests/documentation.test.ts` compares the lint and error pages with the public
IDs, their index links, and the recommended severities. Keep tests for interactions
such as utility lookup, theme switching, and animation controls beside their code.
The compiler packages test compilation and diagnostics. Displayed macro output
comes from the compiler during the website build.

Lint rule and compiler error URLs keep the spelling and case of their IDs. When
an ID is renamed or retired, keep a redirect or a retirement page at its URL.
The lint rule and compiler error index pages link every page in their directory.

`astro.config.ts` lists the sidebar explicitly. Add each new page to the group
that matches its reader task. The **Macros** group lists its pages explicitly:
`style()` comes first, and the other macros follow in alphabetical order.
`style.ref()` and the ref methods come last. Add each new macro page to
that list. Directory groups under **Reference** pick up new at-rule, lint, and
error pages automatically.

Set `PUBLIC_WEBSITE_URL` and `PUBLIC_PLAYGROUND_URL` to the two deployment origins
before building for publication. They default to `https://stillwind.dev/` and
`https://play.stillwind.dev/`. The website origin controls metadata,
agent indexes, and sitemaps. The playground origin controls links to the playground.
Build and content-sync task caches include both variables.

## Writing documentation

The site documents Stillwind for its users. Contracts for maintainers stay in the
repository's `docs/` folder. These rules apply to the site in addition to the
repository's writing guidelines.

### Reader

Write for one reader: a developer who knows Tailwind CSS v4 and builds an
application with a bundler and a component framework. The reader also lets AI
agents write code, and the reader wants that styling code to stay in order.
Do not explain Tailwind basics. Do explain where Stillwind differs from Tailwind.

### Sidebar groups

Getting started stays at `/docs/` and appears first, outside the sidebar groups.
The groups follow the order in which this reader learns Stillwind:

| Group                   | Reader question                                     |
| ----------------------- | --------------------------------------------------- |
| Introduction            | Why use Stillwind, and how do I migrate?            |
| Writing styles          | How do I write the styles that I need every day?    |
| Keeping styles in order | How does Stillwind keep the codebase consistent?    |
| Concepts                | How does Stillwind compile and order styles?        |
| Integrations            | How do I connect Stillwind to my build and runtime? |
| Reference               | What exactly does this API, rule, or error do?      |

Put a page in the group whose question it answers. A guide that shows a
Stillwind-specific check, such as theme restrictions or element references,
belongs in **Keeping styles in order** even when it also teaches an API.

### Guide pages

Start each guide with the reader's goal. When the goal is hard to reach in
Tailwind, state that problem in one or two sentences before the solution.
Then show code, explain what the code produces, and list constraints last.

Show the common path only. Move edge cases, generated JavaScript, and complete
input rules to the reference page, and link to it at the end.

### Reference pages

Keep reference pages precise and complete. Order each page as follows: what the
API does, an example, accepted inputs, behavior, and constraints.

Start lint rule and compiler error pages with the cause and its effect. Put
`Reported` and `Fixed` next when actual code or API inputs show the problem and
its correction. Follow them with `How to fix`. Put rule options and
`LintRuleSummary` last.

Wrap the fences in `Reported` with `DiagnosticExample` and pass the rule name or
error code as `code`. The component runs the example at build time. It annotates
lint ranges and displays compiler errors in a separate block. Configuration and
project files come from the page's `Configuration` and `Project files` sections.

On compiler error pages about project settings, show a complete
`stillwind.config.ts` file with the `defineConfig` import, default export, and
`projects` array with `css` and `files`. Diagnostic examples read that default
export. Keep lint rule settings as focused fragments when the surrounding text
already identifies their location.

The `Fixed` example keeps the intent of the `Reported` example. For example, a
fix for an arbitrary color moves the color into a theme token instead of
choosing an unrelated palette color. Preserve values, conditions, and side
effects. Include the CSS or configuration that an example needs. When the
intent is ambiguous, state the assumption and explain the alternatives.

Code fences contain source code, API data, or actual program output. Write
descriptions of hypothetical states as prose. For host errors, explain what an
application developer can do first, then describe the contract for a custom
integration. Omit example pairs when there is no useful code correction, such
as a compiler bug or an exhausted identifier space.

Test authored facts against their source, including public IDs and preset
defaults. Avoid tests that pin wording or markup, repeat the compiler's tests,
or compile the same example again merely to compare it with its own output.

### Vocabulary

Write in terms of what the reader writes and sees. Use compiler terms only on
reference pages that need them, and define each term where it first appears.

| Avoid in guides                        | Write instead                                               |
| -------------------------------------- | ----------------------------------------------------------- |
| lowers, lowering                       | compiles to                                                 |
| materialize, materialized value        | store in a variable, use as a value                         |
| contribution lowers as empty           | the utility produces no CSS                                 |
| condition lowers as false              | the condition never matches                                 |
| declaration site, declaration identity | each `style.ref()` call creates one name                    |
| provider, consumer                     | the element that defines the name, the element that uses it |
| standalone analysis                    | the CLI and the editor                                      |
| producer module                        | the module that contains the style                          |

### Facts in one place

State each shared fact on one page and link to it from other pages. For example,
the page about checks in the editor and CI explains that builds do not run lint
rules. Other pages link to that explanation instead of repeating it.

### Caveats

Add a caveat only where a reader is likely to make the mistake. Do not answer
questions that the page does not raise, such as how the CLI writes files or
which internal optimizations exist. Do not write "This is A, not B" to defend
the design.

### Comparisons with Tailwind CSS

Stillwind builds on Tailwind CSS. When a page compares them, describe each
difference as a result of their designs, not as a flaw of Tailwind. Tailwind
reads class names as strings, so it needs no build step and accepts classes
anywhere. In return, it cannot check how those strings relate to each other.
Stillwind compiles `style()` calls, so it needs a build tool and can run those
checks.

Do not write that Tailwind fails silently, that nothing stops a mistake, or that
its users lose track of their code.

### Index pages

Index pages help the reader choose a page. Group entries by the reader's
concern and give each entry a one-line description. Group lint rules by what
they catch, such as invalid input, conflicting styles, design system policies,
and preferred spelling. Group compiler errors by the reader's next action:
fix the source, fix the configuration, or report a bug.

## Theme

`src/styles/theme.css` defines the shared theme with blue neutrals and indigo actions.
Its static tokens remain available to authored CSS and compiled examples. Use
semantic colors such as `bg-background`, `bg-surface`, `text-foreground`,
`text-muted`, `border-border`, and `bg-action text-action-foreground`. Use `accent`
for focus indicators and `danger`, `warning`, and `success` for status. Each color
uses `light-dark()` so its public name stays the same in both schemes.

The landing page uses the `page` container width, and documentation pages use the
wider `docs` container width. Navigation between pages uses cross-document view
transitions of type `page`, which `src/styles/site.css` enables only without a reduced-motion
preference. `SiteHeader` and the documentation navigation name themselves with
`style.viewTransition()`, so the header resizes between the two widths and the
navigation stays in place between documentation pages. The root element keeps
its default transition name; removing it would skip the cross-fade of the page.

`SiteHeader` stays in document flow at the top of the page. After it leaves the
viewport, it floats at the top of the viewport. The landing page shows the floating
header from the start of the demonstrations. Documentation pages show it while the
reader scrolls up and hide it after scrolling down, after a jump farther than the
viewport, and after navigation to a fragment. The root element has
`data-floating-header` while a floating header is shown, and the documentation
navigation moves below the header while this attribute is present.

The documentation navigation stores its open groups and scroll position in
`sessionStorage` when a page is hidden. The next documentation page restores
them before its first paint. Groups that contain the current page always open.

Links at the end of each page follow the sidebar order. The previous page appears
on the left, and the next page appears on the right. Nimbus resolves both links
from the same sidebar tree that the layout renders.

The header cycles through system, light, and dark preferences. System follows
the browser's color scheme, including when JavaScript is disabled. Explicit
choices persist in local storage and apply in the head before page content.

| Tokens                                                                    | Contract                                                        |
| ------------------------------------------------------------------------- | --------------------------------------------------------------- |
| `--spacing`                                                               | A 4px base at the default root size; half steps supply 2px gaps |
| `gutter`, `columns`, `control`, `panel`, `stack`, `section` spacing       | Page padding, column gaps, and shared component spacing         |
| `docs-gutter` spacing                                                     | Page padding and column gaps of the documentation layout        |
| `page`, `docs`, `navigation`, `outline`, `prose` containers               | Landing and documentation widths, side columns, and line length |
| `inline`, `control`, `panel` radii                                        | 3px for inline code, 4px for controls, 8px for panels           |
| `inline` breakpoint                                                       | 40rem: horizontal groups and larger navigation                  |
| `sidebar` breakpoint                                                      | 52.5rem: side navigation and paired content                     |
| `outline` breakpoint                                                      | 70rem: the page outline and full demonstration layout           |
| `docs-outline` breakpoint                                                 | 85rem: the outline column of documentation pages                |
| `label`, `caption`, `ui`, `small`, `body`, `lead` text                    | 10, 12, 13, 14, 16, and 18px at the default root size           |
| `heading-small`, `heading`, `title`, `section`, `display` text            | Heading roles; large titles scale within defined limits         |
| `code`, `code-inline`, `code-display`, `code-output`, `code-feature` text | Reading and demonstration code roles                            |

The `demo` breakpoint is 20rem, the minimum width for interactive demonstrations.
Use named text sizes and `@variant` blocks in authored CSS rules.
Use `text-(length:--text-ui)` when a font size must preserve the line height
from another rule.
Use named variants such as `sidebar:flex-row` in macro inputs. Presentation
controllers pass queries from `style.media()` to `matchMedia()`.
The `pin-story*` and `pin-features*` variants define each demonstration's minimum height.
Height and motion conditions govern whether demonstrations can pin and animate;
their thresholds are independent of the layout widths. Measured animation
coordinates and illustration dimensions remain local to their presentation.

## Presentation contracts

`src/lib/landing/code-morph.ts` measures source tokens, and `src/lib/landing/scroll.ts` supplies
normalized progress. CSS interpolates measured positions and visibility.
Only spaces and variant colons divide utility strings, so wrappers keep a
variant and its utility on one line. Reports identify utilities through source
ranges. Controllers release browser subscriptions when disposed.

`src/lib/landing/features/` separates source examples, presentation, progress,
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
