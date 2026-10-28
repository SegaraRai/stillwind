import browserConfig from "@stillwind-internal/e2e-harness/browser-config";
import { defineConfig, mergeConfig } from "vite-plus";

export const browserTests = [
  "tests/css-call.test.ts",
  "tests/cva.test.ts",
  "tests/framework-helpers.test.ts",
  "tests/ref-scopes.test.ts",
  "tests/vanilla.test.ts",
];

export default mergeConfig(
  browserConfig(),
  defineConfig({
    root: import.meta.dirname,
    test: {
      name: "css-browser",
      include: browserTests,
    },
  }),
);
