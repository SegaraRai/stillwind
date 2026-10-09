import browserConfig from "@stillwind-internal/e2e-harness/browser-config";
import { defineConfig, mergeConfig } from "vite-plus";

export default mergeConfig(
  browserConfig(),
  defineConfig({
    root: import.meta.dirname,
    test: {
      name: "css-browser",
      include: ["tests/cva.test.ts", "tests/ref-scopes.test.ts"],
    },
  }),
);
