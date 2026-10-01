import { fixtureTasks } from "@stillwind/e2e-harness/config";
import { stillwind } from "@stillwind/integration/vite";
import { nitro } from "nitro/vite";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  run: {
    tasks: fixtureTasks("vite"),
  },
  plugins: lazyPlugins(async () => {
    const { solidStart } = await import("@solidjs/start/config");
    return [
      stillwind({
        css: "root.css",
        types: false,
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
    fs: {
      allow: [process.cwd()],
    },
  },
  build: {
    sourcemap: true,
  },
});
