import { pluginReact } from "@rsbuild/plugin-react";
import { pluginStillwind } from "@stillwind/integration/rsbuild";

export default {
  plugins: [
    pluginStillwind({
      css: "src/styles.css",
    }),
    pluginReact(),
  ],
  source: {
    entry: {
      index: "./src/main.tsx",
    },
  },
  html: {
    template: "./index.html",
  },
  server: {
    port: Number(process.env.STILLWIND_E2E_PORT || 5173),
    strictPort: true,
    host: "localhost",
  },
  output: {
    sourceMap: {
      js: "source-map",
      css: true,
    },
  },
};
