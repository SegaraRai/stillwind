import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [react()],
  base: "./",
  server: {
    port: 5179,
    strictPort: true,
  },
  run: {
    tasks: {
      generate: {
        command: "",
        dependsOn: ["@stillwind-internal/tools-projects#generate:fixtures:projects"],
      },
      dev: {
        command: "vp dev",
        cache: false,
        dependsOn: ["generate"],
      },
      build: {
        command: "vp build",
        dependsOn: ["generate"],
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: [],
      },
    },
  },
});
