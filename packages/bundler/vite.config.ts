import { defineConfig } from "vite-plus";
export default defineConfig({
  run: {
    tasks: {
      build: { command: "node --check loader.cjs", dependsOn: ["@stillwind/host#build"] },
      typecheck: { command: "tsc --noEmit" },
    },
  },
});
