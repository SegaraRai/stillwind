import { pluginVue } from "@rsbuild/plugin-vue";
import { pluginStillwind } from "@stillwind/integration/rsbuild";

export default {
  plugins: [
    pluginStillwind({
      css: "src/styles.css",
    }),
    pluginVue(),
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
    minify: process.env.STILLWIND_E2E_PROFILE === "unminified" ? false : undefined,
    sourceMap: {
      js: "source-map",
      css: true,
    },
  },
};
