import { fixtureTasks } from "@stillwind/e2e-harness/config";
import { stillwind } from "@stillwind/integration/vite";
import react from "@vitejs/plugin-react";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  run: {
    tasks: fixtureTasks("vite"),
  },
  plugins: lazyPlugins(async () => [
    stillwind({
      css: "src/styles.css",
      types: false,
      emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
    }),
    react(),
  ]),
});
