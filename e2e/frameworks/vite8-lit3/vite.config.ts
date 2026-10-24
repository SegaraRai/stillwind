import { fixtureTasks } from "@stillwind-internal/e2e-harness/config";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  run: {
    tasks: fixtureTasks("vite", import.meta.dirname),
  },
  plugins: lazyPlugins(async () => {
    const { stillwind } = await import("@stillwind/integration/vite");
    return [stillwind({ automatic: false })];
  }),
});
