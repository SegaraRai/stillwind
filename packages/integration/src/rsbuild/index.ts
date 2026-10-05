import { fileURLToPath } from "node:url";

import type { StillwindOptions } from "../options.ts";
import { StillwindPlugin } from "../webpack/plugin.ts";
import { stillwindRules } from "../webpack/rules.ts";

export type { StillwindOptions } from "../options.ts";

/**
 * Adapts existing CSS rules for Stillwind document carriers and source-map composition.
 *
 * The function excludes document carriers from matching CSS rules and inserts the Stillwind map
 * bridge before each Lightning CSS loader entry.
 *
 * @param rules - Mutable Rsbuild rule list, including any nested `oneOf` and `rules` lists.
 */
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

/**
 * Creates an Rsbuild plugin that installs Stillwind compilation and CSS rules.
 *
 * @param options - Compiler configuration containing a CSS entrypoint, shared by the generated
 * loader rules.
 * @returns A plugin whose setup hook adjusts Rspack rules and registers a `StillwindPlugin`
 * instance.
 */
function pluginStillwind(options: StillwindOptions) {
  return {
    name: "stillwind",
    setup(api: { modifyRspackConfig(callback: (config: any) => any): void }) {
      api.modifyRspackConfig((config) => {
        config.module ??= {};
        config.module.rules ??= [];
        configureCssRules(config.module.rules);
        (config.plugins ??= []).push(new RsbuildStillwindPlugin(options));
        return config;
      });
    },
  };
}

/**
 * A Stillwind plugin that places producer compilation within the Rsbuild framework loader chain.
 */
class RsbuildStillwindPlugin extends StillwindPlugin {
  /**
   * Appends Stillwind rules after framework pre-loaders and before normal JSX lowering.
   *
   * Vue query blocks are excluded from the script rule. Document carriers use native CSS modules so
   * rebuilding them does not rerun the extraction loader.
   *
   * @param rules - Mutable Rspack rule list already configured by Rsbuild.
   */
  protected override configureRules(rules: unknown[]): void {
    const [javascript, ...stylesheets] = stillwindRules(this.options);
    // Framework pre-loaders must split routes/server functions before CSS
    // ownership is determined. Appending a normal loader still runs before SWC.
    delete javascript.enforce;
    const script = {
      ...javascript,
      resourceQuery: {
        not: [/[?&]vue(?:[=&]|$)/],
      },
    };
    // Document carriers are rebuilt after graph linking. Native CSS modules avoid
    // running Rsbuild's extraction loader again during that rebuild.
    rules.push(
      script,
      ...stylesheets.map((rule) =>
        rule.test.test("stillwind-webpack.css")
          ? {
              ...rule,
              type: "css",
            }
          : rule,
      ),
    );
  }
}

export { pluginStillwind };
