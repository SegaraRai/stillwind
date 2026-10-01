import { defineConfig } from "vite-plus";

export default defineConfig({
  run: {
    tasks: {
      typecheck: {
        command: "tsc --noEmit",
      },
      e2e: {
        command: "vp test --run --config acceptance.config.ts",
        dependsOn: ["@stillwind/integration#build"],
        cache: false,
      },
    },
  },
});
