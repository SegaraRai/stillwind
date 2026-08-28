import { fileURLToPath, URL } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig, type UserConfig } from "vite";
import { stillwind } from "@stillwind/vite";

type VitePlusConfig = UserConfig & { run: { tasks: Record<string, unknown> } };

export default defineConfig({
  plugins: [
    react(),
    stillwind({
      theme: fileURLToPath(new URL("./styles/default-theme.css", import.meta.url)),
    }),
  ],
  run: {
    tasks: {
      dev: { command: "vite" },
      build: { command: "vite build" },
      check: { command: "tsc --noEmit", dependsOn: ["build"] },
    },
  },
} as VitePlusConfig);
