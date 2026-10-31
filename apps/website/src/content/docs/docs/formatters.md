---
title: Formatters
slug: docs/formatters
description: Sort utilities together with Prettier, Oxfmt, or another document formatter.
---

Stillwind sorts utilities, and a document formatter controls layout and whitespace.
How you combine them depends on the formatter:

| Formatter          | Setup                                                                  |
| ------------------ | ---------------------------------------------------------------------- |
| Prettier           | [The Prettier plugin](#prettier) sorts during formatting               |
| Oxfmt or another   | [Run `stillwind fmt` first](#other-formatters), then run the formatter |
| Any, in the editor | [Code Actions](/docs/editors#sort-and-fix-on-save) sort on save        |

All of them read the [`sort` field](/docs/configuration#sorting) of
`stillwind.config.ts` and use the same order. Sorting runs whatever the severity of
[prefer-sorted-utilities](/docs/lints/prefer-sorted-utilities) is. To stop all
sorting, set `sort: false`.

Create `stillwind.config.ts` with [Getting started](/docs) first.

## Prettier

`prettier-plugin-stillwind` sorts Stillwind inputs before Prettier formats a
document. Install it with Prettier 3.7 or a newer 3.x version:

```sh
vp i -D prettier prettier-plugin-stillwind
```

```js title="prettier.config.mjs"
export default {
  plugins: ["prettier-plugin-stillwind"],
};
```

JavaScript, JSX, TypeScript, TSX, CSS, and Vue work without other plugins. For
Svelte and Astro, install their Prettier plugins, and load them before the
Stillwind plugin:

```js title="prettier.config.mjs"
export default {
  plugins: ["prettier-plugin-svelte", "prettier-plugin-astro", "prettier-plugin-stillwind"],
};
```

### Configuration

The plugin uses the nearest `stillwind.config.*` above each file. The
`stillwindConfig` Prettier option selects another configuration, with a path
relative to the working directory. The plugin reloads the configuration for each
formatting request.

A file with no configuration, or outside the `files` of its project, keeps ordinary
Prettier formatting. An invalid explicit configuration fails the formatting
request, and so does incomplete or conflicting analysis.

### What the plugin changes

The plugin only sorts. To replace utilities with their canonical spellings, run
`stillwind lint --fix`.

- `prettier-ignore` protects the following syntax node from sorting.
- Range formatting protects the source outside the range.
- Suppression comments control lint findings and fixes, and they do not stop sorting.
- Sorting keeps the spelling of each token.

## Other formatters

Run `stillwind fmt` first. It sorts utilities and keeps the source layout, and the
formatter then controls whitespace:

```sh
vp exec stillwind fmt
vp exec oxfmt
```

In CI, check both without saving files:

```sh
vp exec stillwind fmt --check
vp exec oxfmt --check
```

See [the CLI reference](/docs/cli#stillwind-fmt) for the options of `fmt`.
