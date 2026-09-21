import { fileURLToPath, URL } from "node:url";

import { stillwind } from "@stillwind/integration/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

const projectRoot = fileURLToPath(new URL(".", import.meta.url));

export default defineConfig({
  root: projectRoot,
  plugins: [
    react(),
    stillwind({
      theme: fileURLToPath(new URL("./styles/default-theme.css", import.meta.url)),
    }),
  ],
  run: {
    tasks: {
      dev: { command: "vp dev", dependsOn: ["@stillwind/host#build"] },
      build: { command: "vp build", dependsOn: ["@stillwind/host#build"] },
      check: { command: "tsc --noEmit", dependsOn: ["build"] },
    },
  },
});
