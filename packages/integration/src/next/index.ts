import { fileURLToPath } from "node:url";

import type { NextConfig } from "next";
import type { Configuration } from "webpack";

import type { StillwindOptions } from "../options.ts";
import { createRules, loaderPath, createLoaderOptions } from "../turbopack/rules.ts";

export type { StillwindOptions } from "../options.ts";

const loader = loaderPath("loader.js");
const cssLoader = fileURLToPath(
  new URL("./css-loader.js", import.meta.resolve("@stillwind/integration/next")),
);

/**
 * Adds Stillwind rules while preserving the application's Next configuration and webpack hook.
 *
 * @param config - Existing Next configuration, including optional Turbopack rules and webpack hook.
 * @param options - Compiler settings with a CSS entrypoint. Loader options must be serializable.
 * @returns A copy with Stillwind rules appended to both bundler configurations. The original
 * webpack hook runs before Stillwind appends its rules.
 * @throws If a CSS entrypoint is missing or naming callbacks cannot be serialized.
 */
function withStillwind<T extends object>(config: T, options: StillwindOptions): T;

function withStillwind(config: NextConfig = {}, options?: StillwindOptions) {
  const loaderOptions = {
    ...createLoaderOptions(options!, []),
    extensions: config.pageExtensions ?? ["tsx", "ts", "jsx", "js"],
  };
  const rules = { ...config.turbopack?.rules };
  for (const [pattern, rule] of Object.entries(createRules(loaderOptions, cssLoader))) {
    const previous = rules[pattern];
    rules[pattern] = previous ? [...(Array.isArray(previous) ? previous : [previous]), rule] : rule;
  }
  return {
    ...config,
    turbopack: {
      ...config.turbopack,
      rules,
    },
    webpack(
      webpackConfig: Configuration,
      context: Parameters<NonNullable<NextConfig["webpack"]>>[1],
    ) {
      const result: Configuration = config.webpack
        ? config.webpack(webpackConfig, context)
        : webpackConfig;
      result.module ??= {};
      result.module.rules ??= [];
      result.module.rules.push(
        {
          test: /\.[cm]?[jt]sx?$/,
          exclude: /node_modules/,
          enforce: "pre",
          use: [{ loader, options: loaderOptions }],
        },
        {
          test: /stillwind-turbopack\.css$/,
          use: [{ loader: cssLoader, options: loaderOptions }],
        },
      );
      return result;
    },
  };
}

export { withStillwind };
