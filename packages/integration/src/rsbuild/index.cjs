const path = require("node:path");
const { stillwindRules } = require("../shared/rules.cjs");
const { StillwindPlugin } = require("../shared/emission.cjs");
const cssMapLoader = path.join(__dirname, "../shared/css-map-loader.cjs");

function preserveLightningMaps(rules) {
  for (const rule of rules) {
    if (!rule || typeof rule !== "object") continue;
    if (Array.isArray(rule.use)) {
      rule.use = rule.use.flatMap((use) => {
        const name = typeof use === "string" ? use : use?.loader;
        return name === "builtin:lightningcss-loader" ? [{ loader: cssMapLoader }, use] : [use];
      });
    }
    preserveLightningMaps(rule.oneOf ?? []);
    preserveLightningMaps(rule.rules ?? []);
  }
}

function pluginStillwind(options) {
  return {
    name: "stillwind",
    setup(api) {
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
module.exports = { pluginStillwind };
