# Framework projection validation

## Re-run the automated checks

Run from the repository root with the workspace lockfile installed:

```powershell
vp i --frozen-lockfile
vp run @stillwind/host#test
vp run @stillwind/vite#test
$env:ASTRO_TELEMETRY_DISABLED = '1'
vp run @stillwind/vite#test:frameworks
```

The checks cover different boundaries:

| Task                              | Evidence                                                                                                                                                                                                                   |
| --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `@stillwind/host#test`            | Actual converters, native source edits, and framework compiler acceptance; Vue quotes and two script scopes, Svelte module/instance scopes, Astro server/client scripts, explicit CSS imports, and failed-edit diagnostics |
| `@stillwind/vite#test`            | Raw SFC hook input, immutable CSS IDs, query subrequest exclusion, ordinary JS regression tests, and an actual build executing module CSS values and runtime helpers                                                       |
| `@stillwind/vite#test:frameworks` | Official Vue and Svelte Vite plugins and a complete Astro production build; automatic CSS contains the requested red/blue utilities, while explicitly imported green CSS remains a value until the component applies it    |

The production fixtures use Vue 3.5.42 / plugin-vue 6.0.8, Svelte 5.57.0 / vite-plugin-svelte 7.3.0, and Astro 7.3.2, as resolved in `pnpm-lock.yaml`. Projection uses the separately pinned converters in `@stillwind/projection`; Astro's projection converter is `@astrojs/compiler` 4.0.0.

Astro's own compiler is a `pre` plugin. Stillwind also sets the transform hook's `order` to `pre`, ensuring it sees the original SFC before Astro compiles it. Framework query subrequests are excluded. The official integration type declarations refer to Vite directly; the integration tests bridge that declaration boundary to Vite+ and verify the actual runtime build.

## Browser and HMR acceptance

Start all three local fixtures:

```powershell
$env:ASTRO_TELEMETRY_DISABLED = '1'
vp run @stillwind/vite#dev:frameworks
```

The command prints each fixture directory and serves Vue at `http://127.0.0.1:5174`, Svelte at `http://127.0.0.1:5175`, and Astro at `http://127.0.0.1:5176`. Each server has its own dependency cache. Stop the command with Ctrl+C. Fixtures are generated under the ignored `packages/vite/.framework-test-*` directories.

On 2026-09-10, all three pages were opened and inspected in a real browser. These observations are separate from the build assertions:

| Action                                                                    | Observed result in Vue, Svelte, and Astro                                                                                     |
| ------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Open the page                                                             | Target color `rgb(201, 22, 33)`; explicit CSS value color `rgb(17, 155, 44)`; authored `external`/`explicit` classes retained |
| Click Toggle                                                              | Target changed to `rgb(11, 44, 205)` through Vue reactivity, a Svelte rune, or Astro's client script                          |
| Edit `root.css` colors                                                    | Without manually reloading, the target changed to `rgb(140, 30, 180)` and the explicit CSS value to `rgb(230, 120, 20)`       |
| Replace the target's `style(...)` expression with `({class: 'external'})` | Generated class removed; target color became `rgb(0, 0, 0)`                                                                   |
| Restore the macro expression                                              | Generated class and `rgb(140, 30, 180)` color returned after HMR pruning                                                      |
| Replace `root.css` with invalid `@theme {`                                | Last successful target and explicit CSS colors stayed visible                                                                 |
| Restore valid CSS                                                         | Original red/green colors returned without a manual reload                                                                    |

Vue/Svelte use their HMR implementations; Astro may reload the page as part of its update handling. The assertions above do not claim preservation of component state during every kind of update.

To retain production-build fixtures for inspection, set `STILLWIND_KEEP_FRAMEWORK_FIXTURES=1` before running `test:frameworks`. The task prints successful fixture paths instead of deleting them.

## Scope

The host does not yet query checker type facts for projected input. The normal JS/TS checker path remains enabled. SFC source parsing, framework normalization, original-span validation, native editing, source maps, and CSS import placement are exercised independently of that deferred checker integration.

These fixtures validate the Vite integration. Other bundler loaders and additional framework-specific features require their own acceptance checks; a passing converter test alone is not evidence for those boundaries.
