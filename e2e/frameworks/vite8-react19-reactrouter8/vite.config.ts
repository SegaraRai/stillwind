import { reactRouter } from "@react-router/dev/vite";
import { fixtureTasks } from "@stillwind/e2e-harness/config";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  run: {
    tasks: fixtureTasks("react-router", import.meta.dirname),
  },
  plugins: lazyPlugins(async () => {
    const { stillwind } = await import("@stillwind/integration/vite");
    return [
      stillwind({
        css: "root.css",
        types: false,
        emission: "split",
      }),
      reactRouter(),
    ];
  }),
  build: {
    sourcemap: true,
  },
  server: {
    port: Number(process.env.STILLWIND_E2E_PORT || 5173),
    strictPort: true,
  },
});
