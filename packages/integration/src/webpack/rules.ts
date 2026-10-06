import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";

import type { InlineConfig } from "@stillwind/config";

/**
 * Derives the webpack rule identifier used by inline SFC loader requests.
 *
 * @param config - The same configuration values registered on the loader rule. JSON serialization
 * omits callbacks, so namingRevision must distinguish changed naming implementations.
 * @returns A stillwind-prefixed SHA-256 hash of the serialized configuration.
 * @throws If the configuration cannot be serialized as JSON.
 */
export function loaderOptionsId(config: InlineConfig): string {
  return "stillwind-" + createHash("sha256").update(JSON.stringify(config)).digest("hex");
}

/** webpack-compatible producer and source-map rules, also consumed by Rspack. */
export interface StillwindRule {
  test: RegExp;
  exclude?: RegExp;
  enforce?: "pre" | "post";
  use: (
    | string
    | {
        loader: string;
        /** Discovery and inline settings retained by the rule and evaluated by its producer loader. */
        options: InlineConfig;
        /** Registered rule identity referenced by requests that retain naming callbacks. */
        ident: string;
      }
  )[];
}

/**
 * Builds producer, document CSS, and SFC source-map loader rules without evaluating configuration.
 *
 * @param config - Discovery and inline settings shared by producer rules. The object is retained.
 * @returns Rules using absolute paths in the installed integration package. The host provides its
 * CSS pipeline and registers StillwindPlugin when automatic document CSS is required.
 * @throws If loader resolution or configuration serialization fails.
 */
export function stillwindRules(config: InlineConfig): StillwindRule[] {
  return [
    {
      test: /\.[cm]?[jt]sx?$/,
      exclude: /node_modules/,
      enforce: "pre",
      use: [
        {
          loader: fileURLToPath(
            new URL("./loader.js", import.meta.resolve("@stillwind/integration/webpack")),
          ),
          options: config,
          ident: loaderOptionsId(config),
        },
      ],
    },
    {
      test: /\.(?:astro|svelte|vue)$/,
      exclude: /node_modules/,
      enforce: "pre",
      use: [
        {
          loader: fileURLToPath(
            new URL("./loader.js", import.meta.resolve("@stillwind/integration/webpack")),
          ),
          options: config,
          ident: loaderOptionsId(config),
        },
      ],
    },
    {
      test: /stillwind-webpack\.css$/,
      use: [
        fileURLToPath(
          new URL("./css-loader.js", import.meta.resolve("@stillwind/integration/webpack")),
        ),
      ],
    },
    {
      // Vue inline match resources select script rules with a synthetic .vue.ts suffix.
      test: /\.(?:astro|svelte|vue)(?:\.[cm]?[jt]sx?)?$/,
      exclude: /node_modules/,
      enforce: "post",
      use: [
        fileURLToPath(
          new URL("./source-map-loader.js", import.meta.resolve("@stillwind/integration/webpack")),
        ),
      ],
    },
  ];
}
