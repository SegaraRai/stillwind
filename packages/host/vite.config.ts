import { defineConfig } from "vite-plus";
export default defineConfig({
  run: {
    tasks: {
      build: { command: "tsc -p tsconfig.json", dependsOn: ["@stillwind/native#build"] },
      typecheck: { command: "tsc --noEmit" },
      test: { command: "node --test test/*.test.mts", dependsOn: ["build"] },
    },
  },
});
