import { fixtureTasks } from "@stillwind/e2e-harness/config";
import { stillwind } from "@stillwind/integration/vite";
import vue from "@vitejs/plugin-vue";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  build: {
    sourcemap: true,
    minify: process.env.STILLWIND_E2E_PROFILE === "unminified" ? false : undefined,
    cssMinify: process.env.STILLWIND_E2E_PROFILE === "unminified" ? false : undefined,
  },
  run: {
    tasks: fixtureTasks("vite"),
  },
  plugins: lazyPlugins(async () => [
    stillwind({
      css: "src/styles.css",
      types: false,
      emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
    }),
    vue(),
  ]),
});
