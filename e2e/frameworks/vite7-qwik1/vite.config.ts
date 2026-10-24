import { qwikCity } from "@builder.io/qwik-city/vite";
import { qwikVite } from "@builder.io/qwik/optimizer";
import { fixtureTasks } from "@stillwind-internal/e2e-harness/config";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  run: {
    tasks: fixtureTasks("vite", import.meta.dirname),
  },
  plugins: lazyPlugins(async () => {
    const { stillwind } = await import("@stillwind/integration/vite");
    return [
      qwikCity(),
      stillwind(),
      // The hover probe changes layout during Qwik's image diagnostics window.
      qwikVite({
        devTools: {
          imageDevTools: false,
        },
      }),
    ];
  }),
});
