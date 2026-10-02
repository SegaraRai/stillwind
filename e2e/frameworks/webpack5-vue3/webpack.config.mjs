import { createRequire } from "node:module";
import path from "node:path";

import { StillwindPlugin, stillwindRules } from "@stillwind/integration/webpack";
import HtmlPlugin from "html-webpack-plugin";
import { VueLoaderPlugin } from "vue-loader";

const require = createRequire(import.meta.url);
const root = process.cwd();

export default {
  context: root,
  cache: {
    type: "filesystem",
    cacheDirectory: path.join(root, "cache"),
  },
  optimization: {
    minimize: process.env.STILLWIND_E2E_PROFILE === "unminified" ? false : undefined,
  },
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
    new StillwindPlugin(),
    new HtmlPlugin({
      template: "./index.html",
    }),
    new VueLoaderPlugin(),
  ],
  module: {
    rules: [
      ...stillwindRules({
        css: "src/styles.css",
        root,
      }),
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
      {
        test: /\.vue$/,
        loader: require.resolve("vue-loader"),
      },
    ],
  },
};
