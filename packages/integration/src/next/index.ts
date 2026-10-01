import { fileURLToPath } from "node:url";

import type { StillwindOptions } from "../options.ts";
import { createRules, loaderPath, createLoaderOptions } from "../turbopack/rules.ts";

export type { StillwindOptions } from "../options.ts";

const loader = loaderPath("loader.js");
const cssLoader = fileURLToPath(
  new URL("./css-loader.js", import.meta.resolve("@stillwind/integration/next")),
);

function withStillwind<T extends object>(config: T, options: StillwindOptions): T;

function withStillwind(config: any = {}, options?: StillwindOptions) {
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
    webpack(webpackConfig: any, context?: any) {
      const result = config.webpack ? config.webpack(webpackConfig, context) : webpackConfig;
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
