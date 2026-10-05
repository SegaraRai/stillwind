import browserConfig from "@stillwind-internal/e2e-harness/browser-config";
import { defineConfig, mergeConfig } from "vite-plus";

export default mergeConfig(
  browserConfig(),
  defineConfig({
    root: import.meta.dirname,
    test: {
      name: "css-browser",
      include: ["tests/ref-scopes.test.ts"],
    },
  }),
);
