import config from "@stillwind-internal/e2e-harness/acceptance-config";
import { defineConfig, mergeConfig } from "vite-plus";

import { browserTests } from "./vite.browser.config.ts";

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
            exclude: browserTests,
          },
        },
        "./vite.browser.config.ts",
      ],
    },
  }),
);
