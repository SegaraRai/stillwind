import { fixtureTasks } from "@stillwind/e2e-harness/config";
import { stillwind } from "@stillwind/integration/vite";
import vueJsx from "@vitejs/plugin-vue-jsx";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  run: {
    tasks: fixtureTasks(),
  },
  plugins: lazyPlugins(() => [
    stillwind({
      css: "src/styles.css",
      types: false,
      emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
    }),
    vueJsx(),
  ]),
});
