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
    {
      test: /\.(vue|svelte|astro)$/,
      exclude: /node_modules/,
      enforce: "pre",
      use: [{ loader, options }],
    },
    { test: /stillwind-carrier\.css$/, use: [cssLoader] },
    {
      test: /\.(vue|svelte|astro)$/,
      exclude: /node_modules/,
      enforce: "post",
      use: [path.join(__dirname, "source-map-loader.cjs")],
    },
  ];
}

module.exports = { stillwindRules };
