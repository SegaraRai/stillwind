import { fixtureTasks } from "@stillwind/e2e-harness/config";
import solid from "vite-plugin-solid";
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
      solid(),
    ];
  }),
});
