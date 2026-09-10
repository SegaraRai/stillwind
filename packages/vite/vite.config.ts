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
      "test:frameworks": {
        command: "node --test test/framework-builds.mts",
        dependsOn: ["@stillwind/host#build", "typecheck"],
        cache: false,
      },
      "dev:frameworks": {
        command: "node test/framework-dev.mts",
        dependsOn: ["@stillwind/host#build", "typecheck"],
        cache: false,
      },
    },
  },
});
