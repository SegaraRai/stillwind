/**
 * Sorts recognized Stillwind inputs before the selected Prettier parser runs. Place this plugin
 * after language plugins. The Node analysis process evaluates stillwind.config.ts and theme CSS;
 * the adapter uses unsaved text and leaves file writes to Prettier.
 */

import path from "node:path";

import { findFormatterConfig } from "@stillwind/analysis/node";
import type { Parser, Plugin } from "prettier";

import { builtinParsers, createParsers } from "./plugin.ts";
import { sortBuffer } from "./session.ts";

export { options } from "./plugin.ts";

const bases: Record<string, Parser> = { ...builtinParsers };

for (const name of ["prettier-plugin-svelte", "prettier-plugin-astro"] as const) {
  try {
    const plugin = (await import(name)) as Plugin;
    Object.assign(bases, plugin.parsers);
  } catch (error) {
    if (
      !(
        error instanceof Error &&
        "code" in error &&
        error.code === "ERR_MODULE_NOT_FOUND" &&
        error.message.includes(`'${name}'`)
      )
    ) {
      throw error;
    }
  }
}

export const parsers: Record<string, Parser> = createParsers(bases, (filepath, current) => {
  const file = path.resolve(filepath);
  const config = findFormatterConfig(file, current.stillwindConfig);
  return config ? (input) => sortBuffer(config, { ...input, file }) : undefined;
});
