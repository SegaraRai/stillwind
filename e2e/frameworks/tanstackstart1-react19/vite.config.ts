import { fixtureTasks } from "@stillwind-internal/e2e-harness/config";
import { tanstackStart } from "@tanstack/react-start/plugin/vite";
import react from "@vitejs/plugin-react";
import { nitro } from "nitro/vite";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  run: {
    tasks: fixtureTasks("vite", import.meta.dirname),
  },
  plugins: lazyPlugins(async () => {
    const { stillwind } = await import("@stillwind/integration/vite");
    return [
      tanstackStart(),
      stillwind({
        emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
      }),
      nitro(),
      react(),
    ];
  }),
  nitro: {
    preset: "node_server",
  },
});
