import { fileURLToPath } from "node:url";

import type { RsbuildPlugin } from "@rsbuild/core";
import type { RspackOptions } from "@rspack/core";
import type { InlineConfig } from "@stillwind/config";

import { StillwindPlugin } from "../webpack/plugin.ts";
import { stillwindRules } from "../webpack/rules.ts";

export type { InlineConfig as IntegrationConfig } from "@stillwind/config";

/** Rsbuild plugin that uses only the hook for updating the Rspack configuration. */
export interface StillwindRsbuildPlugin extends RsbuildPlugin {
  /**
   * Registers Stillwind's configuration changes through the host's Rspack hook.
   *
   * @param api - Host API that accepts a modifier for the mutable Rspack configuration.
   */
  setup(api: {
    modifyRspackConfig(callback: (options: RspackOptions) => RspackOptions): void;
  }): void;
}

/**
 * Adapts existing CSS rules for Stillwind document carriers and source-map composition.
 *
 * The function excludes document carriers from matching CSS rules and inserts the Stillwind map
 * bridge before each Lightning CSS loader entry.
 *
 * @param rules - Mutable Rsbuild rule list, including any nested `oneOf` and `rules` lists.
 */
function configureCssRules(rules: NonNullable<NonNullable<RspackOptions["module"]>["rules"]>) {
  for (const rule of rules) {
    if (!rule || typeof rule !== "object") {
      continue;
    }
    if (rule.test instanceof RegExp && rule.test.test("stillwind-webpack.css")) {
      rule.exclude = [rule.exclude, /stillwind-webpack\.css$/].filter(
        (condition): condition is NonNullable<typeof condition> => Boolean(condition),
      );
    }
    if (Array.isArray(rule.use)) {
      rule.use = rule.use.flatMap((use) => {
        const name =
          typeof use === "string"
            ? use
            : typeof use === "object" && use !== null
              ? use.loader
              : undefined;
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
 * @param config - Configuration discovery and inline overrides shared by the generated rules.
 * @returns A plugin whose setup hook adjusts Rspack rules and registers a `StillwindPlugin`
 * instance.
 */
export function pluginStillwind(config: InlineConfig = {}): StillwindRsbuildPlugin {
  return {
    name: "stillwind",
    setup(api) {
      api.modifyRspackConfig((options) => {
        options.module ??= {};
        options.module.rules ??= [];
        configureCssRules(options.module.rules);
        options.plugins ??= [];
        options.plugins.push(new RsbuildStillwindPlugin(config));
        return options;
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
    const [javascript, ...stylesheets] = stillwindRules(this.config);
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
