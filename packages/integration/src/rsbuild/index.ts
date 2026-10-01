import { fileURLToPath } from "node:url";

import type { StillwindOptions } from "../options.ts";
import { StillwindPlugin } from "../webpack/emission.ts";
import { stillwindRules } from "../webpack/rules.ts";

export type { StillwindOptions } from "../options.ts";

function configureCssRules(rules: any[]) {
  for (const rule of rules) {
    if (!rule || typeof rule !== "object") {
      continue;
    }
    if (rule.test instanceof RegExp && rule.test.test("stillwind-webpack.css")) {
      rule.exclude = [rule.exclude, /stillwind-webpack\.css$/].filter(Boolean);
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
    configureCssRules(rule.oneOf ?? []);
    configureCssRules(rule.rules ?? []);
  }
}

function pluginStillwind(options: StillwindOptions) {
  return {
    name: "stillwind",
    setup(api: { modifyRspackConfig(callback: (config: any) => any): void }) {
      api.modifyRspackConfig((config) => {
        config.module ??= {};
        config.module.rules ??= [];
        configureCssRules(config.module.rules);
        const [javascript, ...stylesheets] = stillwindRules(options);
        // Framework pre-loaders must split routes/server functions before CSS
        // ownership is determined. Appending a normal loader still runs before SWC.
        delete javascript.enforce;
        const script = { ...javascript, resourceQuery: { not: [/[?&]vue(?:[=&]|$)/] } };
        // Document carriers are rebuilt after graph linking. Native CSS modules avoid
        // running Rsbuild's extraction loader again during that rebuild.
        config.module.rules.push(
          script,
          ...stylesheets.map((rule) =>
            rule.test.test("stillwind-webpack.css") ? { ...rule, type: "css" } : rule,
          ),
        );
        (config.plugins ??= []).push(new StillwindPlugin());
        return config;
      });
    },
  };
}

export { pluginStillwind };
