import { createRequire } from "node:module";
import path from "node:path";

import { HtmlRspackPlugin as HtmlPlugin } from "@rspack/core";
import { StillwindPlugin } from "@stillwind/integration/rspack";

const require = createRequire(import.meta.url);
const root = process.cwd();

export default {
  context: root,
  mode: process.env.NODE_ENV === "production" ? "production" : "development",
  entry: "./src/main.tsx",
  devtool: "source-map",
  experiments: {
    css: true,
  },
  output: {
    path: path.join(root, "dist"),
    filename: "[name].js",
    publicPath: "/",
  },
  resolve: {
    extensions: [".tsx", ".ts", ".jsx", ".js", ".vue", ".svelte"],
    conditionNames: ["svelte", "browser", "import", "default"],
  },
  plugins: [
    new StillwindPlugin({
      css: "src/styles.css",
      root,
    }),
    new HtmlPlugin({
      template: "./index.html",
    }),
  ],
  module: {
    rules: [
      {
        test: /\.[jt]sx?$/,
        exclude: /node_modules/,
        loader: require.resolve("swc-loader"),
        options: {
          jsc: {
            parser: {
              syntax: "typescript",
              tsx: true,
            },
            target: "es2022",
            transform: {
              react: {
                runtime: "automatic",
                importSource: "react",
              },
            },
          },
        },
      },
      {
        test: /\.css$/,
        type: "css",
      },
    ],
  },
};
