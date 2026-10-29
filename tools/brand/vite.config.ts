import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  run: {
    tasks: {
      "generate:brand": {
        command: "node generate.ts",
        cache: false,
      },
      typecheck: {
        command: "tsc --noEmit",
      },
    },
  },
});
