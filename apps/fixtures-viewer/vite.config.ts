import react from "@vitejs/plugin-react";
import { defineConfig, type UserConfig } from "vite";

type VitePlusConfig = UserConfig & { run: { tasks: Record<string, unknown> } };

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
      dev: { command: "vite", dependsOn: ["data:refresh"] },
      build: { command: "vite build", dependsOn: ["data:check"] },
      check: { command: "tsc --noEmit", dependsOn: ["build"] },
    },
  },
} as VitePlusConfig);
