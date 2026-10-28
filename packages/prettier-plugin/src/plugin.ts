/**
 * Parser wrappers shared by the Node and browser entries. A wrapper sorts recognized Stillwind
 * inputs before the selected Prettier parser runs. The entry supplies the sorting service.
 */

import type { Parser, ParserOptions, Plugin, SupportOptions } from "prettier";
import { parsers as babelParsers } from "prettier/plugins/babel";
import { parsers as flowParsers } from "prettier/plugins/flow";
import { parsers as htmlParsers } from "prettier/plugins/html";
import { parsers as cssParsers } from "prettier/plugins/postcss";
import { parsers as typescriptParsers } from "prettier/plugins/typescript";

import { mapOffset } from "./cursor.ts";
import { ignoredRanges } from "./ignore.ts";

const processed = Symbol("stillwind.preprocessed");

/**
 * Prettier options shared with the delegated parser during one formatting operation.
 */
export type StillwindOptions = ParserOptions & {
  /** Configuration path resolved from cwd. Omission searches above the formatted file. */
  stillwindConfig?: string;
  /** Original UTF-16 cursor position, updated after sorting. A negative value disables mapping. */
  cursorOffset?: number;
  /** Marks an options object whose Stillwind preprocessing has already begun, including nested parses. */
  [processed]?: boolean;
};

/**
 * Original buffer and the ranges that sorting must leave untouched.
 */
export interface SortInput {
  /** Complete original text against which edits are measured. */
  source: string;
  /** Half-open UTF-16 ranges in `source`, from ignore directives and the formatting range. */
  protectedRanges: {
    /** Inclusive original UTF-16 offset. */
    start: number;
    /** Exclusive original UTF-16 offset. */
    end: number;
  }[];
}

/**
 * Sorted buffer and the edits that produced it.
 */
export interface SortOutput {
  /** Complete text after sorting. */
  code: string;
  /** Nonoverlapping replacements in `SortInput.source`, in ascending start order. */
  edits: readonly {
    /** Inclusive original UTF-16 offset. */
    start: number;
    /** Exclusive original UTF-16 offset. */
    end: number;
    /** Replacement text. */
    newText: string;
  }[];
}

/**
 * Selects the sorting service for one file before the wrapper parses it.
 *
 * @param filepath - Filepath supplied by Prettier, before path resolution.
 * @param options - Resolved Prettier options for this formatting operation.
 * @returns A function that sorts the buffer, or `undefined` to keep ordinary Prettier formatting.
 * @throws If the project configuration is invalid. The formatting request fails.
 */
export type SortResolver = (
  filepath: string,
  options: StillwindOptions,
) =>
  | ((input: SortInput) => Promise<SortOutput>)
  | undefined
  | Promise<((input: SortInput) => Promise<SortOutput>) | undefined>;

export const options: SupportOptions = {
  stillwindConfig: {
    type: "path",
    category: "Stillwind",
    description: "Path to stillwind.config.ts. Omission searches above the formatted file.",
  },
};

/**
 * Prettier's built-in parsers that the wrappers delegate to when no preceding plugin applies.
 */
export const builtinParsers: Record<string, Parser> = {
  babel: babelParsers.babel,
  "babel-ts": babelParsers["babel-ts"],
  flow: flowParsers.flow,
  typescript: typescriptParsers.typescript,
  vue: htmlParsers.vue,
  css: cssParsers.css,
};

/**
 * Wraps each parser so that it sorts the original buffer before delegating.
 *
 * @param bases - Fallback parsers keyed by Prettier parser name.
 * @param resolve - Selects the sorting service for each formatted file.
 * @returns Wrapper parsers with the same keys as `bases`.
 */
export function createParsers(
  bases: Record<string, Parser>,
  resolve: SortResolver,
): Record<string, Parser> {
  return Object.fromEntries(
    Object.entries(bases).map(([name, base]) => {
      const wrapper: Parser = {
        ...base,
        /**
         * Sorts the original buffer before delegating language-specific preprocessing.
         *
         * @param text - Original source supplied by Prettier for this parser invocation.
         * @param rawOptions - Resolved Prettier options. The shared processed marker prevents nested
         * invocations from sorting again, and `cursorOffset` is mapped through the sorting edits.
         * Ignore directives and text outside the formatting range are protected from sorting.
         * @returns Sorted text after the delegate's preprocessing, or delegated original text when
         * the file is unsupported, already processed, or has no sorting service.
         * @throws If delegate loading, parsing, ignore extraction, configuration, or sorting fails.
         */
        async preprocess(text, rawOptions) {
          const current = rawOptions as StillwindOptions;
          const delegate = await delegateParser(name, wrapper, base, current);
          /**
           * Applies the delegate's preprocessing with the current shared options.
           *
           * @param source - Original or sorted buffer selected for this formatting invocation.
           * @returns The delegate's synchronous or asynchronous result, or unchanged text when absent.
           * @throws If the delegate fails, synchronously or through its returned promise.
           */
          const preprocess = (source: string) =>
            delegate.preprocess ? delegate.preprocess(source, current) : source;
          const source = text;
          if (current[processed] || !current.filepath || !matchesFile(name, current.filepath)) {
            return preprocess(source);
          }
          current[processed] = true;
          const sort = await resolve(current.filepath, current);
          if (!sort) {
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
          const result = await sort({ source, protectedRanges });
          current.cursorOffset = mapOffset(current.cursorOffset ?? -1, source, result.edits);
          return preprocess(result.code);
        },
        /**
         * Parses the preprocessed buffer using the nearest preceding language plugin.
         *
         * @param text - Buffer after this wrapper's preprocessing.
         * @param rawOptions - Resolved Prettier options shared with the selected delegate.
         * @returns The delegate's AST without changing its representation.
         * @throws If delegate loading or parsing fails.
         */
        async parse(text, rawOptions) {
          const current = rawOptions as StillwindOptions;
          return (await delegateParser(name, wrapper, base, current)).parse(text, current);
        },
      };
      return [name, wrapper];
    }),
  );
}

/**
 * Selects the nearest preceding plugin that supplies the same parser name.
 *
 * @param name - Parser registry key used by this wrapper.
 * @param wrapper - Parser object used to identify this plugin in the resolved plugin list.
 * @param fallback - Built-in or optional language parser used when no preceding parser is found.
 * @param current - Resolved options whose object-valued plugins retain Prettier's plugin order.
 * @returns The selected parser, awaiting its factory when present. An absent wrapper also uses
 * the fallback.
 * @throws If the selected parser factory fails.
 */
async function delegateParser(
  name: string,
  wrapper: Parser,
  fallback: Parser,
  current: StillwindOptions,
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

/**
 * Checks whether a filepath can be analyzed by the selected parser.
 *
 * @param parser - Prettier parser name. Component parsers require their exact lowercase suffix.
 * @param file - Filepath supplied by Prettier, before resolution to an absolute path.
 * @returns Whether the component suffix matches, the CSS parser has a case-insensitive `.css`
 * suffix, or a JS/TS suffix matches without a TypeScript declaration suffix.
 */
function matchesFile(parser: string, file: string): boolean {
  if (parser === "css") {
    return /\.css$/i.test(file);
  }
  if (["vue", "svelte", "astro"].includes(parser)) {
    return file.endsWith(`.${parser}`);
  }
  return /\.[cm]?[jt]sx?$/i.test(file) && !/\.d\.[cm]?ts$/i.test(file);
}
