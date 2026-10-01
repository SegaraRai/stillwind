import { fixtureTasks } from "@stillwind/e2e-harness/config";
import { stillwind } from "@stillwind/integration/vite";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  run: {
    tasks: fixtureTasks("astro"),
  },
  plugins: lazyPlugins(async () => [
    stillwind({
      css: "src/styles.css",
      types: false,
      emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
    }),
  ]),
});
