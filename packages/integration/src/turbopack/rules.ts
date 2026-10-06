import { fileURLToPath } from "node:url";

import type { InlineConfig } from "@stillwind/config";

import type { TurbopackLoaderOptions } from "./project.ts";

/**
 * Serializable inline settings and entry selection for standalone Turbopack rules.
 * Naming callbacks can be supplied by stillwind.config.ts, which each loader evaluates.
 */
export interface IntegrationConfig extends Omit<InlineConfig, "naming"> {
  /** Nonempty list of entry paths, absolute or relative to the resolved project root. Globs are not expanded. */
  entries: string[];
  /** Defaults to development when NODE_ENV is development, otherwise release. */
  mode?: "development" | "release";
}

/** Host rule that maps source files or the shared stylesheet carrier to Stillwind loaders. */
export interface TurbopackRule {
  /** Limits the producer rule to application modules rather than foreign package modules. */
  condition?: {
    not: "foreign";
  };
  loaders: {
    /** Absolute path to an installed Stillwind loader. */
    loader: string;
    /** Serializable settings forwarded to the isolated loader invocation. */
    options: TurbopackLoaderOptions;
  }[];
  /** Forces the stylesheet carrier's output to be processed as CSS. */
  as?: string;
}

/**
 * Creates producer and shared stylesheet rules for a host's turbopack.rules configuration.
 *
 * @param config - Entry paths, optional mode, configuration file selection, and serializable
 * compiler overrides. File evaluation occurs in loaders after the bundler starts.
 * @returns Rules with independent Stillwind loaders. The host supplies its CSS output pipeline.
 * @throws If entries is empty or contains blank paths, inline naming callbacks are supplied,
 * or installed loader paths cannot be resolved.
 */
export function stillwindRules(config: IntegrationConfig): Record<string, TurbopackRule> {
  if (
    !Array.isArray(config.entries) ||
    !config.entries.length ||
    config.entries.some((entry) => typeof entry !== "string" || !entry.trim())
  ) {
    throw new Error("Stillwind Turbopack requires a non-empty list of application entry files");
  }
  const { entries, mode, ...compiler } = config;
  return createRules(createLoaderOptions(compiler, entries, mode));
}

/**
 * Captures the application directory and serializable settings for isolated loader workers.
 *
 * @param compiler - Discovery settings and inline overrides. Naming callbacks must be defined in
 * the configuration file, since loader transport cannot serialize inline functions.
 * @param entries - Entry paths copied into the loader options. Next can supply an empty list and
 * discover routes later. Paths are resolved after the loader evaluates the project root.
 * @param mode - Compilation mode. Omission uses development only when NODE_ENV is development.
 * @returns Loader options containing process.cwd() as the application directory for discovery.
 * @throws If compiler supplies inline naming callbacks.
 */
export function createLoaderOptions(
  compiler: InlineConfig,
  entries: string[],
  mode?: string,
): TurbopackLoaderOptions {
  if (compiler.naming) {
    throw new Error(
      "Turbopack loader options must be serializable; naming callbacks are not supported",
    );
  }
  return {
    compiler,
    root: process.cwd(),
    entries: [...entries],
    mode: mode ?? (process.env.NODE_ENV === "development" ? "development" : "release"),
  };
}

/**
 * Locates a built loader beside the installed Turbopack entrypoint.
 *
 * @param file - Filename relative to that entrypoint, such as loader.js or css-loader.js.
 * @returns The absolute filesystem path. Existence of the selected file is not checked.
 * @throws If the installed Turbopack entrypoint cannot be resolved.
 */
export function loaderPath(file: string): string {
  return fileURLToPath(
    new URL(`./${file}`, import.meta.resolve("@stillwind/integration/turbopack")),
  );
}

/**
 * Creates source and stylesheet carrier rules from captured loader options.
 *
 * @param options - Serializable settings retained by both rules without copying.
 * @param cssLoader - Absolute stylesheet loader path. Next supplies its route-discovery wrapper.
 * Omission selects the standalone Turbopack stylesheet loader.
 * @returns Rules keyed by source and carrier filename patterns. Configuration is not evaluated here.
 * @throws If an installed loader path cannot be resolved.
 */
export function createRules(
  options: TurbopackLoaderOptions,
  cssLoader = loaderPath("css-loader.js"),
): Record<string, TurbopackRule> {
  return {
    "*.{js,jsx,ts,tsx,mjs,cjs}": {
      condition: {
        not: "foreign",
      },
      loaders: [
        {
          loader: loaderPath("loader.js"),
          options,
        },
      ],
    },
    "stillwind-turbopack.css": {
      loaders: [
        {
          loader: cssLoader,
          options,
        },
      ],
      as: "*.css",
    },
  };
}
