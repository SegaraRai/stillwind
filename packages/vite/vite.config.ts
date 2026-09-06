import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      typecheck: {
        command: "tsc --noEmit",
      },
      test: {
        command: "node --test test/*.test.mts",
        dependsOn: ["@stillwind/native#build", "typecheck"],
      },
    },
  },
});
