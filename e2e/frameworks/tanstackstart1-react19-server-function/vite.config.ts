import { fixtureTasks } from "@stillwind/e2e-harness/config";
import { stillwind } from "@stillwind/integration/vite";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import react from "@vitejs/plugin-react";
import { nitro } from "nitro/vite";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  run: {
    tasks: fixtureTasks("vite"),
  },
  plugins: lazyPlugins(() => [
    tanstackStart(),
    stillwind({
      css: "root.css",
      types: false,
      emission: "split",
    }),
    nitro(),
    react(),
  ]),
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
