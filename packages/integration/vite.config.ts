import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      build: {
        command: "node --check src/shared/loader.cjs",
        dependsOn: ["@stillwind/host#build"],
      },
      typecheck: { command: "tsc --noEmit", dependsOn: ["build"] },
      test: {
        command: "node --test test/vite/*.test.mts test/webpack/*.test.mts test/*.test.mts",
        dependsOn: ["typecheck"],
        cache: false,
      },
      "test:vite": {
        command: "node --test test/vite/*.test.mts",
        dependsOn: ["typecheck"],
        cache: false,
      },
      "test:webpack": {
        command: "node --test test/webpack/*.test.mts",
        dependsOn: ["typecheck"],
        cache: false,
      },
      "test:frameworks": {
        command: "node --test test/vite/framework-builds.mts",
        dependsOn: ["typecheck"],
        cache: false,
      },
      "dev:frameworks": {
        command: "node test/vite/framework-dev.mts",
        dependsOn: ["typecheck"],
        cache: false,
      },
    },
  },
});
