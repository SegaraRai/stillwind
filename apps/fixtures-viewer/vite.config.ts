import react from "@vitejs/plugin-react";
import { defineConfig } from "vite-plus";

export default defineConfig({
  plugins: [react()],
  base: "./",
  run: {
    tasks: {
      "data:refresh": {
        command: "node ../../tools/review-fixtures.mts",
        dependsOn: ["@stillwind/native#build"],
      },
      "data:check": {
        command: "node ../../tools/review-fixtures.mts --check",
        dependsOn: ["@stillwind/native#build"],
      },
      dev: { command: "vp dev", dependsOn: ["data:refresh"] },
      build: { command: "vp build", dependsOn: ["data:check"] },
      check: { command: "tsc --noEmit", dependsOn: ["build"] },
    },
  },
});
