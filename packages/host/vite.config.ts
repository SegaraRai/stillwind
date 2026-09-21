import { defineConfig } from "vite-plus";
export default defineConfig({
  run: {
    tasks: {
      build: {
        command: "tsc -p tsconfig.json",
        dependsOn: ["@stillwind/native#build", "@stillwind/projection#build"],
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: ["@stillwind/projection#build"],
      },
      test: {
        command: "node --test test/*.test.mts",
        dependsOn: ["build"],
      },
    },
  },
});
