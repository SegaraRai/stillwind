import { fixtureTasks } from "@stillwind-internal/e2e-harness/config";
import vueJsx from "@vitejs/plugin-vue-jsx";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  run: {
    tasks: fixtureTasks("vite", import.meta.dirname),
  },
  plugins: lazyPlugins(async () => {
    const { stillwind } = await import("@stillwind/integration/vite");
    return [
      stillwind({
        css: "src/styles.css",
        types: false,
        emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
      }),
      vueJsx(),
    ];
  }),
});
