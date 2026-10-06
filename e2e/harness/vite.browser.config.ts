import { defineConfig, mergeConfig } from "vite-plus";

import browserConfig from "./src/browser-config.ts";

export default mergeConfig(
  browserConfig(),
  defineConfig({
    root: import.meta.dirname,
    optimizeDeps: {
      include: [
        "@stillwind/language-server > vscode-languageserver-textdocument",
        "@stillwind/language-server > vscode-languageserver/browser",
        "@stillwind/language-server > vscode-languageserver",
      ],
    },
    test: {
      name: "wasm-browser",
      include: ["src/language-server-browser.test.ts", "src/wasm-browser.test.ts"],
      testTimeout: 30_000,
    },
  }),
);
