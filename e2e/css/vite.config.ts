import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      typecheck: {
        command: "tsc --noEmit",
      },
      "test:browser": {
        command: "vp test --run --config vite.browser.config.ts",
        dependsOn: ["@stillwind/native#collect:local"],
        cache: false,
      },
      e2e: {
        command: "vp test --run --config acceptance.config.ts",
        dependsOn: ["@stillwind/integration#build", "@stillwind/native#collect:local"],
        cache: false,
      },
    },
  },
});
