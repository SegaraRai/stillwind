import { fileURLToPath } from "node:url";

import type { SerializableInlineConfig } from "@stillwind/config";
import type { NextConfig } from "next";
import type { Configuration } from "webpack";

import { createLoaderOptions, createRules, loaderPath } from "../turbopack/rules.ts";

export type { SerializableInlineConfig as IntegrationConfig } from "@stillwind/config";

const loader = loaderPath("loader.js");

const cssLoader = fileURLToPath(
  new URL("./css-loader.js", import.meta.resolve("@stillwind/integration/next")),
);

/**
 * Adds Stillwind rules while preserving the application's Next configuration and webpack hook.
 *
 * @param nextConfig - Existing Next configuration, including optional Turbopack rules and webpack hook.
 * @param config - Configuration discovery and inline compiler overrides. Inline values must be
 * serializable; callbacks can be declared in the configuration file.
 * @returns A copy with Stillwind rules appended to both bundler configurations. The original
 * webpack hook runs before Stillwind appends its rules.
 * @throws If inline naming callbacks cannot be serialized. Loaders report configuration errors
 * when the bundler evaluates the settings.
 */
export function withStillwind<T extends object>(
  nextConfig: T,
  config?: SerializableInlineConfig,
): T;

export function withStillwind(nextConfig: NextConfig = {}, config: SerializableInlineConfig = {}) {
  const loaderOptions = {
    ...createLoaderOptions(config, []),
    extensions: nextConfig.pageExtensions ?? ["tsx", "ts", "jsx", "js"],
  };
  const rules = { ...nextConfig.turbopack?.rules };
  for (const [pattern, rule] of Object.entries(createRules(loaderOptions, cssLoader))) {
    const previous = rules[pattern];
    rules[pattern] = previous ? [...(Array.isArray(previous) ? previous : [previous]), rule] : rule;
  }
  return {
    ...nextConfig,
    turbopack: {
      ...nextConfig.turbopack,
      rules,
    },
    webpack(
      webpackConfig: Configuration,
      context: Parameters<NonNullable<NextConfig["webpack"]>>[1],
    ) {
      const result: Configuration = nextConfig.webpack
        ? nextConfig.webpack(webpackConfig, context)
        : webpackConfig;
      result.module ??= {};
      result.module.rules ??= [];
      result.module.rules.push(
        {
          test: /\.[cm]?[jt]sx?$/,
          exclude: /node_modules/,
          enforce: "pre",
          use: [
            {
              loader,
              options: loaderOptions,
            },
          ],
        },
        {
          test: /stillwind-turbopack\.css$/,
          use: [
            {
              loader: cssLoader,
              options: loaderOptions,
            },
          ],
        },
      );
      return result;
    },
  };
}
