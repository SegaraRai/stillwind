const path = require("node:path");
const loader = path.join(__dirname, "loader.cjs");
const cssLoader = path.join(__dirname, "css-loader.cjs");
const cssMapLoader = path.join(__dirname, "css-map-loader.cjs");

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

function stillwindRules(options) {
  return [
    {
      test: /\.[cm]?[jt]sx?$/,
      exclude: /node_modules/,
      enforce: "pre",
      use: [{ loader, options }],
    },
    { test: /stillwind-carrier\.css$/, use: [cssLoader] },
  ];
}

function withStillwind(config = {}, options) {
  if (!options?.css) throw new Error("Stillwind requires a CSS entrypoint");
  const rules = { ...config.turbopack?.rules };
  for (const [pattern, rule] of Object.entries({
    "*.{js,jsx,ts,tsx,mjs,cjs}": { loaders: [{ loader, options }] },
    "stillwind-carrier.css": { loaders: [cssLoader], as: "*.css" },
  })) {
    const previous = rules[pattern];
    rules[pattern] = previous ? [...(Array.isArray(previous) ? previous : [previous]), rule] : rule;
  }
  return {
    ...config,
    turbopack: {
      ...config.turbopack,
      rules,
    },
    webpack(webpackConfig, context) {
      const result = config.webpack ? config.webpack(webpackConfig, context) : webpackConfig;
      result.module ??= {};
      result.module.rules ??= [];
      result.module.rules.push(...stillwindRules(options));
      return result;
    },
  };
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
        return config;
      });
    },
  };
}
module.exports = { stillwindRules, withStillwind, pluginStillwind };
