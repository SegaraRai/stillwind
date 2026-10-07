/**
 * Sorts recognized Stillwind inputs before the selected Prettier parser runs. Place this plugin
 * after language plugins. The Node analysis process evaluates stillwind.config.ts and theme CSS;
 * the adapter uses unsaved text and leaves file writes to Prettier.
 */

import path from "node:path";

import { findFormatterConfig } from "@stillwind/analysis/node";
import type { Parser, ParserOptions, Plugin, SupportOptions } from "prettier";
import { parsers as babelParsers } from "prettier/plugins/babel";
import { parsers as flowParsers } from "prettier/plugins/flow";
import { parsers as htmlParsers } from "prettier/plugins/html";
import { parsers as typescriptParsers } from "prettier/plugins/typescript";

import { mapOffset } from "./cursor.ts";
import { ignoredRanges } from "./ignore.ts";
import { sortBuffer } from "./session.ts";

const processed = Symbol("stillwind.preprocessed");

type Options = ParserOptions & {
  stillwindConfig?: string;
  cursorOffset?: number;
  [processed]?: boolean;
};

export const options: SupportOptions = {
  stillwindConfig: {
    type: "path",
    category: "Stillwind",
    description: "Path to stillwind.config.ts. Omission searches above the formatted file.",
  },
};

const bases: Record<string, Parser> = {
  babel: babelParsers.babel,
  "babel-ts": babelParsers["babel-ts"],
  flow: flowParsers.flow,
  typescript: typescriptParsers.typescript,
  vue: htmlParsers.vue,
};

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

export const parsers: Record<string, Parser> = Object.fromEntries(
  Object.entries(bases).map(([name, base]) => {
    const wrapper: Parser = {
      ...base,
      async preprocess(text, rawOptions) {
        const current = rawOptions as Options;
        const delegate = await delegateParser(name, wrapper, base, current);
        const preprocess = (source: string) =>
          delegate.preprocess ? delegate.preprocess(source, current) : source;
        const source = text;
        if (current[processed] || !current.filepath || !matchesFile(name, current.filepath)) {
          return preprocess(source);
        }
        current[processed] = true;
        const file = path.resolve(current.filepath);
        const config = findFormatterConfig(file, current.stillwindConfig);
        if (!config) {
          return preprocess(source);
        }
        const ast = await delegate.parse(source, {
          ...current,
          originalText: source,
        });
        const protectedRanges = await ignoredRanges(ast, delegate, source, current);
        if (current.rangeStart > 0) {
          protectedRanges.push({
            start: 0,
            end: current.rangeStart,
          });
        }
        if (current.rangeEnd < source.length) {
          protectedRanges.push({
            start: current.rangeEnd,
            end: source.length,
          });
        }
        const result = await sortBuffer(config, {
          file,
          source,
          protectedRanges,
        });
        current.cursorOffset = mapOffset(current.cursorOffset ?? -1, source, result.edits);
        return preprocess(result.code);
      },
      async parse(text, rawOptions) {
        const current = rawOptions as Options;
        return (await delegateParser(name, wrapper, base, current)).parse(text, current);
      },
    };
    return [name, wrapper];
  }),
);

async function delegateParser(
  name: string,
  wrapper: Parser,
  fallback: Parser,
  current: Options,
): Promise<Parser> {
  const plugins = current.plugins.filter((plugin): plugin is Plugin => typeof plugin === "object");
  const ownIndex = plugins.findIndex((plugin) => plugin.parsers?.[name] === wrapper);
  for (let index = ownIndex - 1; index >= 0; index--) {
    const parser = plugins[index].parsers?.[name];
    if (parser) {
      const candidate = parser as Parser | (() => Parser | Promise<Parser>);
      return typeof candidate === "function" ? candidate() : candidate;
    }
  }
  return fallback;
}

function matchesFile(parser: string, file: string): boolean {
  if (["vue", "svelte", "astro"].includes(parser)) {
    return file.endsWith(`.${parser}`);
  }
  return /\.[cm]?[jt]sx?$/i.test(file) && !/\.d\.[cm]?ts$/i.test(file);
}
