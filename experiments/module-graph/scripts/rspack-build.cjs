const fs = require("node:fs");
const path = require("node:path");
const { rspack } = require("@rspack/core");
const dir = process.argv[2];
const compiler = rspack({
  context: dir,
  mode: "production",
  optimization: { minimize: false },
  entry: "./main.js",
  cache: {
    type: "persistent",
    storage: { type: "filesystem", directory: path.join(dir, "cache") },
  },
  output: {
    path: path.join(dir, "dist"),
    filename: "main.js",
    cssFilename: "[name].[contenthash].css",
  },
  module: {
    rules: [
      { test: /\.js$/, exclude: /node_modules/, use: [path.join(__dirname, "next-loader.cjs")] },
      { test: /carrier\.css$/, type: "css", use: [path.join(__dirname, "css-loader.cjs")] },
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
          modules: true,
          cachedModules: true,
          chunks: true,
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
