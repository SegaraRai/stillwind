import { defineConfig, mergeConfig } from "vite-plus";

import browserConfig from "./src/browser-config.ts";

export default mergeConfig(
  browserConfig(),
  defineConfig({
    root: import.meta.dirname,
    test: {
      name: "wasm-browser",
      include: ["src/wasm-browser.test.ts"],
    },
  }),
);
