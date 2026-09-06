import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [react()],
  base: "./",
  run: {
    tasks: {
      "data:refresh": {
        command: "",
        dependsOn: ["@stillwind/tools#review:refresh"],
      },
      "data:check": {
        command: "",
        dependsOn: ["@stillwind/tools#review:check"],
      },
      dev: { command: "vp dev", dependsOn: ["data:refresh"] },
      build: { command: "vp build", dependsOn: ["data:check"] },
      check: { command: "tsc --noEmit", dependsOn: ["build"] },
    },
  },
});
