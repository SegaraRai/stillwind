import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      typecheck: { command: "tsc --noEmit" },
      check: { command: "node --test test/*.test.mts", dependsOn: ["typecheck"] },
    },
  },
});
