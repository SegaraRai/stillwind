# Stillwind Prettier plugin

Stillwind compiles Tailwind CSS v4 utilities into component props and static CSS.
It uses theme rules and compiler checks to keep styling code in order, even when
AI agents write much of it.

`prettier-plugin-stillwind` is Stillwind's Prettier plugin. It sorts recognized
Stillwind inputs before Prettier formats a document.

## Install

Install the plugin alongside Prettier 3.7 or newer within the 3.x series:

```sh
vp i -D prettier prettier-plugin-stillwind
```

```js
// prettier.config.mjs
export default {
  plugins: ["prettier-plugin-stillwind"],
};
```

## Languages and plugin order

JavaScript, JSX, TypeScript, TSX, and CSS use Prettier's existing parsers and printers. CSS sorting
uses `sort.applyUtilities` (true by default), `sort.applyStatements`, and `sort.variantBlocks`
(both false by default). Vue uses its built-in parser. Svelte and Astro require their language
plugins to be installed and loaded
before the Stillwind plugin:

```js
export default {
  plugins: ["prettier-plugin-svelte", "prettier-plugin-astro", "prettier-plugin-stillwind"],
};
```

## Configuration and behavior

The plugin uses the nearest `stillwind.config.*` above the file, including its project
scopes, theme CSS, and `sort` options. Sorting runs independently of the
`prefer-sorted-utilities` lint severity. Lint suppression comments control lint
diagnostics and fixes; explicit formatting still sorts.

Create a [Stillwind project configuration](https://github.com/SegaraRai/stillwind/blob/main/packages/core/README.md#configure-a-project)
before formatting. `stillwindConfig` can select an explicit configuration path relative to the working directory.
API callers must supply `filepath`; the plugin analyzes the supplied text, including unsaved
buffers. Files without a discovered configuration or outside its project scopes keep ordinary
Prettier formatting. Invalid explicit configuration and incomplete or conflicting analysis fail
the formatting request. The plugin runs in Node.js and does not provide a browser entry point.

`prettier-ignore` protects the following syntax node from sorting. Range formatting protects
source outside the supplied range. CSS statement and variant movement also preserves the relative
order of competing property writes. Sorting preserves token spellings and source length, and
the adapter keeps cursors attached to uniquely identifiable tokens within reordered literals.
Duplicate or ambiguous tokens retain their relative cursor offset within the replacement.

The plugin only sorts. Canonical utility replacements are available through `stillwind lint --fix`.
Prettier owns saving files. The plugin reloads project configuration for each formatting request.

See [Editor sorting](https://github.com/SegaraRai/stillwind/blob/main/docs/standalone-analysis.md#editor-sorting)
for the shared sorting contract and [Lint policies](https://github.com/SegaraRai/stillwind/blob/main/docs/standalone-analysis.md#utility-lint-policies)
for canonical replacements and other checks.
