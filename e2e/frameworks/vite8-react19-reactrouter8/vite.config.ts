import { reactRouter } from "@react-router/dev/vite";
import { fixtureTasks } from "@stillwind/e2e-harness/config";
import { stillwind } from "@stillwind/integration/vite";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  run: {
    tasks: fixtureTasks("react-router"),
  },
  plugins: lazyPlugins(() => [
    stillwind({
      css: "root.css",
      types: false,
      emission: "split",
    }),
    reactRouter(),
  ]),
  build: {
    sourcemap: true,
  },
  server: {
    port: Number(process.env.STILLWIND_E2E_PORT || 5173),
    strictPort: true,
  },
});
