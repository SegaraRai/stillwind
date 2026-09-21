import { fileURLToPath } from "node:url";

import type { StillwindOptions } from "../options.ts";
import { StillwindPlugin } from "../webpack/emission.ts";
import { stillwindRules } from "../webpack/rules.ts";

export type { StillwindOptions } from "../options.ts";

function preserveLightningMaps(rules: any[]) {
  for (const rule of rules) {
    if (!rule || typeof rule !== "object") {
      continue;
    }
    if (Array.isArray(rule.use)) {
      rule.use = rule.use.flatMap((use: any) => {
        const name = typeof use === "string" ? use : use?.loader;
        return name === "builtin:lightningcss-loader"
          ? [
              {
                loader: fileURLToPath(
                  new URL(
                    "./css-map-loader.js",
                    import.meta.resolve("@stillwind/integration/webpack"),
                  ),
                ),
              },
              use,
            ]
          : [use];
      });
    }
    preserveLightningMaps(rule.oneOf ?? []);
    preserveLightningMaps(rule.rules ?? []);
  }
}

function pluginStillwind(options: StillwindOptions) {
  return {
    name: "stillwind",
    setup(api: { modifyRspackConfig(callback: (config: any) => any): void }) {
      api.modifyRspackConfig((config) => {
        config.module ??= {};
        config.module.rules ??= [];
        preserveLightningMaps(config.module.rules);
        const [javascript, ...stylesheets] = stillwindRules(options);
        // Framework pre-loaders must split routes/server functions before CSS
        // ownership is determined. Appending a normal loader still runs before SWC.
        delete javascript.enforce;
        config.module.rules.push(javascript, ...stylesheets);
        (config.plugins ??= []).push(new StillwindPlugin());
        return config;
      });
    },
  };
}

export { pluginStillwind };
