import { fixtureTasks } from "@stillwind-internal/e2e-harness/config";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  run: {
    tasks: fixtureTasks("vite", import.meta.dirname),
  },
  plugins: lazyPlugins(async () => {
    const { stillwind } = await import("@stillwind/integration/vite");
    const { sveltekit } = await import("@sveltejs/kit/vite");
    const { default: adapter } = await import("@sveltejs/adapter-node");
    return [
      stillwind({
        emission: process.env.STILLWIND_E2E_PROFILE === "split" ? "split" : "single",
      }),
      sveltekit({
        adapter: adapter(),
      }),
    ];
  }),
});
