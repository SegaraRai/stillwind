import { defineConfig } from "vite-plus";

export default defineConfig({
  test: { name: "static-play", include: ["src/**/*.test.ts"], testTimeout: 120_000 },
  run: {
    tasks: {
      "test:e2e": {
        command: "vp test --run",
        cache: false,
        dependsOn: ["@stillwind-internal/apps-play#build:acceptance"],
      },
      typecheck: { command: "tsc --noEmit" },
    },
  },
});
