import { fileURLToPath, URL } from "node:url";

import { stillwind } from "@stillwind/integration/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

import { playground } from "./server/plugin.ts";

const projectRoot = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  root: projectRoot,
  plugins: [
    react(),
    playground("styles/default-theme.css"),
    stillwind({
      theme: fileURLToPath(new URL("./styles/default-theme.css", import.meta.url)),
    }),
  ],
  run: {
    tasks: {
      dev: { command: "vp dev", dependsOn: ["@stillwind/integration#build"] },
      build: { command: "vp build", dependsOn: ["@stillwind/integration#build"] },
      test: {
        command: "node --test test/*.test.mts",
        dependsOn: ["@stillwind/host#build", "stillwind#build"],
        cache: false,
      },
      check: { command: "tsc --noEmit", dependsOn: ["build"] },
    },
  },
});
