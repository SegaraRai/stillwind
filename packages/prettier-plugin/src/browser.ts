/**
 * Browser entry of the Stillwind Prettier plugin. Browsers cannot start the Node analysis
 * process, so the host supplies the sorting service, for example a language server that has
 * the project open. The plugin wraps Prettier's built-in JavaScript, TypeScript, Flow, Vue, and
 * CSS parsers. Svelte and Astro files keep ordinary formatting.
 */

import type { Plugin } from "prettier";

import { builtinParsers, createParsers, options, type SortResolver } from "./plugin.ts";

export type { SortInput, SortOutput, SortResolver, StillwindOptions } from "./plugin.ts";

/**
 * Creates a plugin object that sorts with a host-supplied service.
 *
 * @param resolve - Selects the sorting service for each formatted file. Returning `undefined`
 * keeps ordinary Prettier formatting for that file.
 * @returns A plugin to pass in Prettier's `plugins` option after language plugins.
 */
export function createStillwindPlugin(resolve: SortResolver): Plugin {
  return {
    options,
    parsers: createParsers(builtinParsers, resolve),
  };
}
