import { fixtureTasks } from "@stillwind-internal/e2e-harness/config";
import { nitro } from "nitro/vite";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  run: {
    tasks: fixtureTasks("vite", import.meta.dirname),
  },
  plugins: lazyPlugins(async () => {
    const { stillwind } = await import("@stillwind/integration/vite");
    const { solidStart } = await import("@solidjs/start/config");
    return [
      stillwind({
        css: "root.css",
        emission: "split",
      }),
      solidStart({
        devOverlay: false,
      }),
      nitro(),
    ];
  }),
  nitro: {
    preset: "node_server",
  },
  server: {
    port: Number(process.env.STILLWIND_E2E_PORT || 5173),
    strictPort: true,
  },
  build: {
    sourcemap: true,
  },
});
