const path = require("node:path");
const loader = path.join(__dirname, "loader.cjs");
const cssLoader = path.join(__dirname, "css-loader.cjs");

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
        config.module.rules.push(...stillwindRules(options));
        return config;
      });
    },
  };
}
module.exports = { stillwindRules, withStillwind, pluginStillwind };
