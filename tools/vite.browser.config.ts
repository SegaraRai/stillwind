import browserConfig from "@stillwind/e2e-harness/browser-config";
import { defineConfig, mergeConfig } from "vite-plus";

export default mergeConfig(
  browserConfig(),
  defineConfig({
    root: import.meta.dirname,
    test: {
      name: "tailwind-browser",
      include: ["tailwind/browser.test.ts"],
      globalSetup: ["./tailwind/browser-setup.ts"],
    },
  }),
);
