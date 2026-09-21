const path = require("node:path");
const { stillwindRules } = require("../shared/rules.cjs");
const { StillwindPlugin } = require("../shared/emission.cjs");
const loader = path.join(__dirname, "../shared/loader.cjs");
const cssLoader = path.join(__dirname, "../shared/css-loader.cjs");

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
      (result.plugins ??= []).push(new StillwindPlugin());
      return result;
    },
  };
}

module.exports = { withStillwind };
