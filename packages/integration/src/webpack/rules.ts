import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";

import type { StillwindOptions } from "stillwind/config";

export function loaderOptionsId(options: StillwindOptions): string {
  return "stillwind-" + createHash("sha256").update(JSON.stringify(options)).digest("hex");
}

export interface StillwindRule {
  test: RegExp;
  exclude?: RegExp;
  enforce?: "pre" | "post";
  use: (string | { loader: string; options: StillwindOptions; ident: string })[];
}

export function stillwindRules(options: StillwindOptions): StillwindRule[] {
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
          options,
          ident: loaderOptionsId(options),
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
          options,
          ident: loaderOptionsId(options),
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
