import process from "node:process";

import { fresh } from "@fresh/plugin-vite";
import { stillwind } from "@stillwind/integration/fresh";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [fresh(), stillwind()],
  server: {
    // Deno's Windows file watcher can miss a rapid edit and restore.
    watch:
      process.platform === "win32"
        ? {
            usePolling: true,
            interval: 100,
          }
        : undefined,
  },
});
