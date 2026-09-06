import { fileURLToPath, URL } from "node:url";

import { stillwind } from "@stillwind/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [
    react(),
    stillwind({
      theme: fileURLToPath(new URL("./styles/stillwind.css", import.meta.url)),
    }),
  ],
  run: {
    tasks: {
      dev: { command: "vp dev" },
      build: { command: "vp build" },
      check: { command: "tsc --noEmit", dependsOn: ["build"] },
    },
  },
});
