import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      check: { command: "node --test test/*.test.mts" },
    },
  },
});
