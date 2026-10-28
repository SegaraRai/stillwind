import { testCache } from "@stillwind-internal/tools-workspace/task-cache";
import react from "@vitejs/plugin-react";
import { defineConfig, lazyPlugins } from "vite-plus";

const headers = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
};

export default defineConfig({
  root: import.meta.dirname,
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
      "fmt:example": {
        command:
          "stillwind lint --fix -c example/stillwind.config.ts && stillwind fmt -c example/stillwind.config.ts && prettier --write example",
        dependsOn: ["@stillwind/cli#build", "@stillwind/plugin-iconify#build", "stillwind#build"],
      },
      "check:example": {
        command: "",
        dependsOn: ["check:example:format", "check:example:sort", "check:example:lint"],
      },
      "check:example:format": {
        // The playground formats with Prettier defaults, so the demo uses them instead of vp fmt.
        command: "prettier --check example",
      },
      "check:example:sort": {
        command: "stillwind fmt --check -c example/stillwind.config.ts",
        dependsOn: ["@stillwind/cli#build", "@stillwind/plugin-iconify#build", "stillwind#build"],
      },
      "check:example:lint": {
        command: "stillwind lint --max-warnings=0 -c example/stillwind.config.ts",
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
