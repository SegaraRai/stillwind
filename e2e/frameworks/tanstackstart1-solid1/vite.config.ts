import { fixtureTasks } from "@stillwind-internal/e2e-harness/config";
import { tanstackStart } from "@tanstack/solid-start/plugin/vite";
import { nitro } from "nitro/vite";
import solid from "vite-plugin-solid";
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
        css: "src/styles.css",
        types: false,
        emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
      }),
      nitro(),
      solid({
        ssr: true,
      }),
    ];
  }),
  nitro: {
    preset: "node_server",
  },
});
