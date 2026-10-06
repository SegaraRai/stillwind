import { fixtureTasks } from "@stillwind-internal/e2e-harness/config";
import vue from "@vitejs/plugin-vue";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  build: {
    sourcemap: true,
    minify: process.env.STILLWIND_E2E_PROFILE === "unminified" ? false : undefined,
    cssMinify: process.env.STILLWIND_E2E_PROFILE === "unminified" ? false : undefined,
  },
  run: {
    tasks: fixtureTasks("vite", import.meta.dirname),
  },
  plugins: lazyPlugins(async () => {
    const { stillwind } = await import("@stillwind/integration/vite");
    return [
      stillwind({
        emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
      }),
      vue(),
    ];
  }),
});
