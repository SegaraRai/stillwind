import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["@stillwind/host#build"],
      },
      test: {
        command: "node --test test/*.test.mts",
        dependsOn: ["@stillwind/host#build", "typecheck"],
      },
    },
  },
});
