import config from "@stillwind-internal/e2e-harness/acceptance-config";
import { defineConfig, mergeConfig } from "vite-plus";

export default mergeConfig(
  config,
  defineConfig({
    root: import.meta.dirname,
    test: {
      projects: [
        {
          test: {
            name: "applications",
            include: ["tests/**/*.test.ts"],
            exclude: ["tests/ref-scopes.test.ts", "tests/cva.test.ts"],
          },
        },
        "./vite.browser.config.ts",
      ],
    },
  }),
);
