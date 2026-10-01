import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [react()],
  base: "./",
  run: {
    tasks: {
      "data:refresh": {
        command: "",
        dependsOn: ["@stillwind/tools#projects:refresh"],
      },
      "data:check": {
        command: "",
        dependsOn: ["@stillwind/tools#projects:check"],
      },
      dev: {
        command: "vp dev",
        dependsOn: ["data:refresh"],
      },
      build: {
        command: "vp build",
        dependsOn: ["data:check"],
      },
      check: {
        command: "tsc --noEmit",
        dependsOn: ["build"],
      },
    },
  },
});
