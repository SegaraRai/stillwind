import { defineConfig } from "vite-plus";

export default defineConfig({
  root: import.meta.dirname,
  run: {
    tasks: {
      typecheck: {
        command: "tsc --noEmit",
      },
    },
  },
});
