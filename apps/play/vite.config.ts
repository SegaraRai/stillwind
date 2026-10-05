import { fileURLToPath, URL } from "node:url";

import { testCache } from "@stillwind-internal/tools-workspace/task-cache";
import react from "@vitejs/plugin-react";
import { defineConfig, lazyPlugins } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  server: {
    port: 5177,
    strictPort: true,
  },
  test: {
    name: "play",
    include: ["src/**/*.test.ts", "server/**/*.test.ts"],
    testTimeout: 60_000,
  },
  plugins: lazyPlugins(async () => {
    const { stillwind } = await import("@stillwind/integration/vite");
    const { playground } = await import(new URL("./server/plugin.ts", import.meta.url).href);
    return [
      react(),
      playground("styles/default-theme.css"),
      stillwind({
        theme: fileURLToPath(new URL("./styles/default-theme.css", import.meta.url)),
      }),
    ];
  }),
  run: {
    tasks: {
      dev: {
        command: "vp dev",
        cache: false,
        dependsOn: [
          "@stillwind/host#build",
          "@stillwind/integration#build",
          "@stillwind/native#build",
          "stillwind#build",
        ],
      },
      build: {
        command: "vp build",
        dependsOn: [
          "@stillwind/host#build",
          "@stillwind/integration#build",
          "@stillwind/native#build",
          "stillwind#build",
        ],
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: [
          "@stillwind/host#build",
          "@stillwind/integration#build",
          "@stillwind/native#build",
          "stillwind#build",
        ],
      },
      test: {
        command: "vp test --run",
        cache: testCache,
        dependsOn: [
          "@stillwind/host#build",
          "@stillwind/integration#build",
          "@stillwind/native#build",
          "stillwind#build",
        ],
      },
    },
  },
});
