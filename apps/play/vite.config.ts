import { readFileSync } from "node:fs";

import { testCache } from "@stillwind-internal/tools-workspace/task-cache";
import react from "@vitejs/plugin-react";
import { defineConfig, lazyPlugins } from "vite-plus";

const { version } = JSON.parse(
  readFileSync(new URL("../../packages/core/package.json", import.meta.url), "utf8"),
) as { version: string };

const headers = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
};

export default defineConfig({
  root: import.meta.dirname,
  define: {
    __STILLWIND_VERSION__: JSON.stringify(version),
  },
  publicDir: "dist/assets-static",
  resolve: {
    alias: {
      "monaco-editor": "@codingame/monaco-vscode-editor-api",
      vscode: "@codingame/monaco-vscode-extension-api",
    },
    dedupe: ["@codingame/monaco-vscode-api"],
  },
  server: {
    port: 5177,
    strictPort: true,
    headers,
  },
  preview: {
    headers,
  },
  build: {
    outDir: "dist/site",
    target: "esnext",
    sourcemap: true,
  },
  worker: {
    format: "es",
  },
  optimizeDeps: {
    exclude: [
      "@codingame/monaco-vscode-api",
      "@codingame/monaco-vscode-editor-api",
      "@codingame/monaco-vscode-extension-api",
      // Its WASM binding and thread worker load from URLs relative to the package files.
      "@rolldown/browser",
    ],
  },
  plugins: lazyPlugins(async () => {
    const { stillwind } = await import("@stillwind/integration/vite");
    return [stillwind(), react()];
  }),
  test: {
    name: "play",
    include: ["src/**/*.test.ts"],
    testTimeout: 60_000,
  },
  run: {
    tasks: {
      dev: {
        command: "vp dev",
        cache: false,
        dependsOn: [
          "build:assets",
          "@stillwind/host#build",
          "@stillwind/integration#build",
          "prettier-plugin-stillwind#build",
        ],
      },
      build: {
        command: "vp build",
        dependsOn: [
          "build:assets",
          "@stillwind/host#build",
          "@stillwind/integration#build",
          "prettier-plugin-stillwind#build",
        ],
      },
      "build:acceptance": {
        command: "vp build --base=/play/ --outDir=dist/acceptance",
        dependsOn: [
          "build:assets",
          "@stillwind/host#build",
          "@stillwind/integration#build",
          "prettier-plugin-stillwind#build",
        ],
      },
      "build:assets": {
        command: "node build/assets.ts",
        cache: {
          input: [{ auto: true }, "!dist/**"],
          output: ["dist/assets-static/**"],
        },
        dependsOn: [
          "@stillwind/plugin-iconify#build",
          "@stillwind/vscode-extension#build",
          "stillwind#build",
        ],
      },
      "fmt:examples": {
        command: "node build/examples.ts fix",
        cache: false,
        dependsOn: ["@stillwind/cli#build", "@stillwind/plugin-iconify#build", "stillwind#build"],
      },
      "check:examples": {
        command: "node build/examples.ts check",
        dependsOn: ["@stillwind/cli#build", "@stillwind/plugin-iconify#build", "stillwind#build"],
      },
      typecheck: {
        command: "tsc --noEmit",
        dependsOn: [
          "@stillwind/host#build",
          "@stillwind/integration#build",
          "prettier-plugin-stillwind#build",
        ],
      },
      test: {
        command: "vp test --run",
        cache: testCache,
        dependsOn: ["@stillwind/host#build", "prettier-plugin-stillwind#build"],
      },
    },
  },
});
