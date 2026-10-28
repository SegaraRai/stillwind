import { createRequire } from "node:module";
import path from "node:path";

import { StillwindPlugin } from "@stillwind/integration/webpack";
import HtmlPlugin from "html-webpack-plugin";

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
        loader: require.resolve("babel-loader"),
        options: {
          presets: [
            require.resolve("@babel/preset-typescript"),
            [
              require.resolve("babel-preset-solid"),
              {
                generate: "dom",
                hydratable: false,
              },
            ],
          ],
        },
      },
      {
        test: /\.css$/,
        type: "css",
      },
    ],
  },
};
