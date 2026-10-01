import { fixtureTasks } from "@stillwind/e2e-harness/config";
import { stillwind } from "@stillwind/integration/vite";
import { tanstackStart } from "@tanstack/solid-start/plugin/vite";
import { nitro } from "nitro/vite";
import solid from "vite-plugin-solid";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  run: {
    tasks: fixtureTasks("vite"),
  },
  plugins: lazyPlugins(async () => [
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
  ]),
  nitro: {
    preset: "node_server",
  },
});
