# Stillwind Prettier plugin

`prettier-plugin-stillwind` sorts recognized Stillwind inputs before Prettier formats a document.
The plugin uses the nearest `stillwind.config.ts` above the file, including its project scopes,
theme CSS, and `sort` options. Sorting runs independently of the `sort-utilities` lint severity.
Lint suppression comments control lint diagnostics and fixes; explicit formatting still sorts.

Install the plugin alongside Prettier 3.7 or newer:

```sh
vp i -D prettier prettier-plugin-stillwind
```

```js
// prettier.config.mjs
export default {
  plugins: ["prettier-plugin-stillwind"],
};
```

JavaScript, JSX, TypeScript, and TSX use Prettier's existing parsers and printers. Vue uses its
built-in parser. Svelte and Astro require their language plugins to be installed and loaded
before the Stillwind plugin:

```js
export default {
  plugins: ["prettier-plugin-svelte", "prettier-plugin-astro", "prettier-plugin-stillwind"],
};
```

`stillwindConfig` can select an explicit configuration path relative to the working directory.
API callers must supply `filepath`; the plugin analyzes the supplied text, including unsaved
buffers. Files without a discovered configuration or outside its project scopes keep ordinary
Prettier formatting. Invalid explicit configuration and incomplete or conflicting analysis fail
the formatting request. The plugin runs in Node.js and does not provide a browser entry point.

`prettier-ignore` protects the following syntax node from sorting. Range formatting protects
source outside the supplied range. Sorting preserves token spellings and source length, and
the adapter keeps cursors attached to uniquely identifiable tokens within reordered literals.
Duplicate or ambiguous tokens retain their relative cursor offset within the replacement.

The plugin only sorts. Canonical utility replacements are available through `stillwind lint --fix`.
The plugin never writes files; Prettier owns saving. Analysis runs in an isolated process, reuses
that process for nearby requests, reloads configuration for each request, and releases idle processes.
