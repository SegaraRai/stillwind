const fs = require("node:fs");
const path = require("node:path");
const { rspack } = require("@rspack/core");
const { stillwindRules, StillwindPlugin } = require("@stillwind/integration/rspack");

const dir = process.argv[2];
const compiler = rspack({
  context: dir,
  plugins: [new StillwindPlugin()],
  mode: "production",
  devtool: "source-map",
  optimization: { minimize: false },
  entry: "./main.js",
  cache: {
    type: "persistent",
    storage: {
      type: "filesystem",
      directory: path.join(dir, "cache"),
    },
  },
  output: {
    path: path.join(dir, "dist"),
    filename: "main.js",
    cssFilename: "[name].[contenthash].css",
    publicPath: "/",
  },
  module: {
    rules: [
      ...stillwindRules({
        css: path.join(dir, "root.css"),
        root: dir,
      }),
      {
        test: /\.css$/,
        type: "css",
      },
      {
        test: /\.svg$/,
        type: "asset/resource",
      },
    ],
  },
});

compiler.run((error, stats) => {
  if (error || stats.hasErrors()) {
    console.error(error || stats.toString());
    process.exitCode = 1;
  } else {
    fs.writeFileSync(
      path.join(dir, "stats.json"),
      JSON.stringify(
        stats.toJson({
          all: false,
          assets: true,
          chunks: true,
          modules: true,
          cachedModules: true,
          nestedModules: true,
          orphanModules: true,
          groupModulesByAttributes: false,
          groupModulesByCacheStatus: false,
        }),
        null,
        2,
      ),
    );
  }
  compiler.close((error) => {
    if (error) {
      console.error(error);
      process.exitCode = 1;
    }
  });
});
