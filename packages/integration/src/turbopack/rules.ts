import path from "node:path";
import { fileURLToPath } from "node:url";

import { cssEntrypoint } from "@stillwind/host";
import type { StillwindOptions } from "stillwind/config";

import type { TurbopackLoaderOptions } from "./project.ts";

export interface TurbopackOptions extends Omit<StillwindOptions, "naming"> {
  /** All application entry files, absolute or relative to root. Their static imports are traversed. */
  entries: string[];
  /** Defaults to development when NODE_ENV is development, otherwise release. */
  mode?: "development" | "release";
}

export interface TurbopackRule {
  condition?: { not: "foreign" };
  loaders: { loader: string; options: TurbopackLoaderOptions }[];
  as?: string;
}

/** Returns rules for a host's turbopack.rules configuration. */
export function stillwindRules(options: TurbopackOptions): Record<string, TurbopackRule> {
  if (
    !Array.isArray(options.entries) ||
    !options.entries.length ||
    options.entries.some((entry) => typeof entry !== "string" || !entry.trim())
  ) {
    throw new Error("Stillwind Turbopack requires a non-empty list of application entry files");
  }
  const { entries, mode, ...compiler } = options;
  return createRules(createLoaderOptions(compiler, entries, mode));
}

export function createLoaderOptions(
  compiler: StillwindOptions,
  entries: string[],
  mode?: string,
): TurbopackLoaderOptions {
  cssEntrypoint(compiler ?? {});
  if (compiler.naming) {
    throw new Error(
      "Turbopack loader options must be serializable; naming callbacks are not supported",
    );
  }
  return {
    compiler,
    root: path.resolve(process.cwd(), compiler.root ?? "."),
    entries: [...entries],
    mode: mode ?? (process.env.NODE_ENV === "development" ? "development" : "release"),
  };
}

export function loaderPath(file: string): string {
  return fileURLToPath(
    new URL(`./${file}`, import.meta.resolve("@stillwind/integration/turbopack")),
  );
}

export function createRules(
  options: TurbopackLoaderOptions,
  cssLoader = loaderPath("css-loader.js"),
): Record<string, TurbopackRule> {
  return {
    "*.{js,jsx,ts,tsx,mjs,cjs}": {
      condition: { not: "foreign" },
      loaders: [{ loader: loaderPath("loader.js"), options }],
    },
    "stillwind-turbopack.css": {
      loaders: [{ loader: cssLoader, options }],
      as: "*.css",
    },
  };
}
