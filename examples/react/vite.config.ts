import { fileURLToPath, URL } from "node:url";

import react from "@vitejs/plugin-react";
import { defineConfig, lazyPlugins } from "vite-plus";

const projectRoot = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  root: projectRoot,
  server: {
    port: 5178,
    strictPort: true,
  },
  plugins: lazyPlugins(async () => {
    const { stillwind } = await import("@stillwind/integration/vite");
    return [
      react(),
      stillwind({
        theme: fileURLToPath(new URL("./styles/stillwind.css", import.meta.url)),
      }),
    ];
  }),
  run: {
    tasks: {
      dev: {
        command: "vp dev",
        cache: false,
        dependsOn: ["@stillwind/integration#build"],
      },
      build: {
        command: "vp build",
        dependsOn: ["@stillwind/integration#build"],
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["@stillwind/integration#build"],
      },
    },
  },
});
