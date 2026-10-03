import { fileURLToPath, URL } from "node:url";

import { stillwind } from "@stillwind/integration/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

import { playground } from "./server/plugin.ts";

export default defineConfig({
  root: import.meta.dirname,
  test: {
    name: "play",
    include: ["src/**/*.test.ts", "server/**/*.test.ts"],
    testTimeout: 60_000,
  },
  plugins: [
    react(),
    playground("styles/default-theme.css"),
    stillwind({
      theme: fileURLToPath(new URL("./styles/default-theme.css", import.meta.url)),
    }),
  ],
  run: {
    tasks: {
      dev: {
        command: "vp dev",
        dependsOn: ["@stillwind/integration#build"],
      },
      build: {
        command: "vp build",
        dependsOn: ["@stillwind/integration#build"],
      },
      test: {
        command: "vp test --run",
        dependsOn: ["@stillwind/integration#build"],
        cache: false,
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["@stillwind/integration#build"],
      },
    },
  },
});
